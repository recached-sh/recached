//! Decoding one pipelined request straight from the read buffer.
//!
//! The general path builds a [`Value`] tree from the frame — one allocation for
//! the element array and one per argument — and then [`Command::from_value`]
//! walks it again, upper-casing the command name into a fresh `String` and
//! validating every identifier's UTF-8 twice. On a pipelined `SET` that was
//! five of the fifteen allocations the server made per command, and the
//! completeness check (`Value::frame_len`) walked the frame once more before
//! any of it.
//!
//! [`RequestDecoder::decode`] does one walk over a frame of the shape nearly
//! every client sends — an array of non-null bulk strings — recording each
//! argument as a byte range into the buffer. The ranges live in a vector the
//! decoder keeps between calls, so in the steady state that walk allocates
//! nothing. The command is then built from those slices: the hot commands
//! directly, copying each key and value exactly once into the `Command` that
//! owns it; everything else by handing the same slices to
//! [`Command::from_value`].
//!
//! **Equivalence is the contract.** For every input, `decode` returns exactly
//! what `Value::parse` followed by `Command::from_value` returns — the same
//! command, the same error text, the same `ParseError`, the same byte count.
//! The direct builders handle only the success case of one exact shape per
//! command; any frame they do not fully accept (wrong arity, a non-UTF-8 key,
//! an empty or oversized key, a RESP type other than a bulk string, an
//! incomplete or malformed frame) goes through the general path, which is the
//! one that produces the error. `decoder_matches_the_general_path` checks the
//! equivalence differentially.
//!
//! Nothing borrowed from the input outlives the call: the ranges are offsets,
//! cleared on every call, and the returned `Command` owns its data.

use crate::cmd::{Command, SetOptions, validate_key};
use crate::resp::{MAX_BULK_STRING_BYTES, MAX_TOTAL_MESSAGE_BYTES, ParseError, Value};
use smallvec::SmallVec;
use std::ops::Range;

/// Most arguments a frame may have and still take the single-walk scan.
///
/// A bound on the span vector the decoder retains, not a protocol limit:
/// a longer frame is decoded by the general path, which enforces the real
/// limits.
const MAX_SCANNED_ARGS: usize = 4096;

/// Decodes requests for one connection, reusing its argument buffer.
#[derive(Debug, Default)]
pub struct RequestDecoder {
    spans: Vec<Range<usize>>,
}

/// A decoded frame: the command (or the reason it was refused) and how many
/// bytes of the buffer it occupied.
pub type Decoded = (Result<Command, String>, usize);

/// One of the hot commands, its arguments borrowed from the read buffer.
///
/// Built by [`RequestDecoder::decode_any`] for exactly the frames the owned
/// builder accepts for the same commands — the owned command *is*
/// [`to_command`](Self::to_command) of this — so a server can run it without
/// copying its key and value out of the buffer, and fall back to the owned
/// command when it cannot. It borrows the buffer: it must be run, or turned
/// into a `Command`, before the buffer is compacted or reused.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandRef<'a> {
    Get(&'a str),
    Set(&'a str, &'a [u8]),
    Incr(&'a str),
    Decr(&'a str),
    /// HGET does not validate its key on the general path either.
    HGet(&'a str, &'a str),
    HSet(&'a str, SmallVec<[(&'a str, &'a [u8]); 2]>),
    MGet(SmallVec<[&'a str; 8]>),
    /// 6O-C: `LPUSH key value…`. The values are payloads (any bytes); the
    /// store copies each one once, into the list.
    LPush(&'a str, SmallVec<[&'a [u8]; 4]>),
    /// `RPUSH key value…`, as `LPush`.
    RPush(&'a str, SmallVec<[&'a [u8]; 4]>),
    /// `SADD key member…`. Members are identifiers and must be text; the
    /// store copies a member only when it is new to the set.
    SAdd(&'a str, SmallVec<[&'a str; 4]>),
}

impl CommandRef<'_> {
    /// The owned command, exactly as the owned decoder builds it.
    #[must_use]
    pub fn to_command(&self) -> Command {
        match self {
            CommandRef::Get(key) => Command::Get((*key).to_owned()),
            CommandRef::Set(key, val) => {
                Command::Set((*key).to_owned(), val.to_vec(), SetOptions::default())
            }
            CommandRef::Incr(key) => Command::Incr((*key).to_owned()),
            CommandRef::Decr(key) => Command::Decr((*key).to_owned()),
            CommandRef::HGet(key, field) => Command::HGet((*key).to_owned(), (*field).to_owned()),
            CommandRef::HSet(key, pairs) => Command::HSet(
                (*key).to_owned(),
                pairs
                    .iter()
                    .map(|(field, val)| ((*field).to_owned(), val.to_vec()))
                    .collect(),
            ),
            CommandRef::MGet(keys) => Command::MGet(keys.iter().map(|k| (*k).to_owned()).collect()),
            CommandRef::LPush(key, vals) => {
                Command::LPush((*key).to_owned(), vals.iter().map(|v| v.to_vec()).collect())
            }
            CommandRef::RPush(key, vals) => {
                Command::RPush((*key).to_owned(), vals.iter().map(|v| v.to_vec()).collect())
            }
            CommandRef::SAdd(key, members) => Command::SAdd(
                (*key).to_owned(),
                members.iter().map(|m| (*m).to_owned()).collect(),
            ),
        }
    }

    /// Whether this command writes.
    #[must_use]
    pub fn is_write(&self) -> bool {
        // Exhaustive: a write variant added later must say so here, or it
        // would run as a read past every write gate.
        match self {
            CommandRef::Set(..)
            | CommandRef::Incr(_)
            | CommandRef::Decr(_)
            | CommandRef::HSet(..)
            | CommandRef::LPush(..)
            | CommandRef::RPush(..)
            | CommandRef::SAdd(..) => true,
            CommandRef::Get(_) | CommandRef::HGet(..) | CommandRef::MGet(_) => false,
        }
    }

    /// The key a write mutates (every borrowed write names exactly one).
    #[must_use]
    pub fn write_key(&self) -> Option<&str> {
        match self {
            CommandRef::Set(key, _)
            | CommandRef::Incr(key)
            | CommandRef::Decr(key)
            | CommandRef::HSet(key, _)
            | CommandRef::LPush(key, _)
            | CommandRef::RPush(key, _)
            | CommandRef::SAdd(key, _) => Some(key),
            CommandRef::Get(_) | CommandRef::HGet(..) | CommandRef::MGet(_) => None,
        }
    }
}

/// What [`RequestDecoder::decode_any`] found at the start of the buffer.
#[derive(Debug)]
pub enum DecodedAny<'a> {
    /// A hot command, borrowed, and the bytes its frame occupied.
    Borrowed(CommandRef<'a>, usize),
    /// Anything else, exactly as [`RequestDecoder::decode`] returns it.
    Owned(Result<Decoded, ParseError>),
}

impl RequestDecoder {
    #[must_use]
    pub fn new() -> RequestDecoder {
        RequestDecoder::default()
    }

    /// Decode the frame at the start of `buf`.
    ///
    /// # Errors
    ///
    /// Exactly those of [`Value::parse`]: [`ParseError::Incomplete`] when more
    /// bytes are needed (with the same lower bound), any other variant for a
    /// protocol violation. A frame that parses but is not a valid command is
    /// `Ok` with an `Err` command, as from [`Command::from_value`].
    pub fn decode(&mut self, buf: &[u8]) -> Result<Decoded, ParseError> {
        match self.decode_any(buf) {
            DecodedAny::Borrowed(command, consumed) => Ok((Ok(command.to_command()), consumed)),
            DecodedAny::Owned(decoded) => decoded,
        }
    }

    /// Decode the frame at the start of `buf` in one walk, borrowing the
    /// arguments of a hot command instead of copying them.
    ///
    /// For every input, turning a `Borrowed` result into a command with
    /// [`CommandRef::to_command`] gives exactly what [`decode`](Self::decode)
    /// returns; `Owned` is `decode`'s own result.
    pub fn decode_any<'a>(&'a mut self, buf: &'a [u8]) -> DecodedAny<'a> {
        if let Some(consumed) = self.scan(buf) {
            let args = Args {
                buf,
                spans: &self.spans,
            };
            if let Some(command) = build_ref(args) {
                return DecodedAny::Borrowed(command, consumed);
            }
            let command = build(args).unwrap_or_else(|| args.general());
            return DecodedAny::Owned(Ok((command, consumed)));
        }
        DecodedAny::Owned(Self::decode_general(buf))
    }

    fn decode_general(buf: &[u8]) -> Result<Decoded, ParseError> {
        // Measured first, as the connection loop always did: an incomplete
        // frame is answered without allocating a single element.
        Value::frame_len(buf)?;
        let (value, consumed) = Value::parse(buf)?;
        Ok((Command::from_value(value), consumed))
    }

    /// Walk a complete array of non-null bulk strings, recording each
    /// payload's range. `None` for anything else — another type, a null, an
    /// incomplete or malformed frame — which the general path then handles,
    /// so this never has to produce an error of its own.
    fn scan(&mut self, buf: &[u8]) -> Option<usize> {
        self.spans.clear();
        if buf.first() != Some(&b'*') {
            return None;
        }
        let (header, mut offset) = Value::read_until_crlf(buf)?;
        let count = usize::try_from(Value::parse_header_int(header)?).ok()?;
        if count == 0 || count > MAX_SCANNED_ARGS {
            return None;
        }
        for _ in 0..count {
            let rest = buf.get(offset..)?;
            if rest.first() != Some(&b'$') {
                return None;
            }
            let (header, head_len) = Value::read_until_crlf(rest)?;
            let len = usize::try_from(Value::parse_header_int(header)?).ok()?;
            if len > MAX_BULK_STRING_BYTES {
                return None;
            }
            let start = offset.checked_add(head_len)?;
            let end = start.checked_add(len)?;
            if buf.get(end..end.checked_add(2)?)? != b"\r\n" {
                return None;
            }
            self.spans.push(start..end);
            offset = end + 2;
        }
        // The general parser refuses a frame once it grows past this; such a
        // frame goes there to be refused the same way.
        (offset <= MAX_TOTAL_MESSAGE_BYTES).then_some(offset)
    }
}

/// The scanned arguments of one frame: slices of the input buffer.
#[derive(Clone, Copy)]
struct Args<'a> {
    buf: &'a [u8],
    spans: &'a [Range<usize>],
}

impl<'a> Args<'a> {
    fn len(&self) -> usize {
        self.spans.len()
    }

    fn get(&self, i: usize) -> Option<&'a [u8]> {
        self.buf.get(self.spans.get(i)?.clone())
    }

    fn sub(&self, from: usize) -> Args<'a> {
        Args {
            buf: self.buf,
            spans: self.spans.get(from..).unwrap_or_default(),
        }
    }

    /// An identifier argument: valid UTF-8, else `None`.
    fn text(&self, i: usize) -> Option<&'a str> {
        std::str::from_utf8(self.get(i)?).ok()
    }

    /// A key: text that also passes the general path's key validation.
    fn key(&self, i: usize) -> Option<String> {
        self.key_ref(i).map(str::to_owned)
    }

    /// [`key`](Self::key), borrowed.
    fn key_ref(&self, i: usize) -> Option<&'a str> {
        let key = self.text(i)?;
        validate_key(key).ok()?;
        Some(key)
    }

    fn bytes(&self, i: usize) -> Option<Vec<u8>> {
        self.get(i).map(<[u8]>::to_vec)
    }

    fn name_is(&self, name: &str) -> bool {
        self.get(0)
            .is_some_and(|n| n.eq_ignore_ascii_case(name.as_bytes()))
    }

    /// The general path for these arguments: the `Value` the parser would
    /// have built from the same frame, through `from_value`.
    fn general(&self) -> Result<Command, String> {
        // `map`, not `filter_map`: an exact-size iterator lets `collect`
        // allocate the element array once. Every span came from `scan` over
        // this same buffer, so the `get` never misses.
        let elements = self
            .spans
            .iter()
            .map(|r| Value::BulkString(Some(self.buf.get(r.clone()).unwrap_or_default().to_vec())))
            .collect();
        Command::from_value(Value::Array(Some(elements)))
    }
}

/// Build the command directly when the frame is one of the hot shapes and
/// every argument passes; `None` hands it to the general path.
///
/// Each arm accepts only an exact arity the general path accepts with the
/// same result. Looser shapes the general path tolerates (`GET k extra`),
/// options (`SET k v EX 10`), and every error case go there instead, so the
/// two can only agree.
/// The borrowed hot shapes: `None` for anything else, including any frame
/// of these commands that the owned builder would not accept directly.
fn build_ref(args: Args<'_>) -> Option<CommandRef<'_>> {
    let name = args.get(0)?;
    let n = args.len();
    // Dispatch on length first: one integer compare rejects most names
    // before any byte comparison.
    Some(match (name.len(), n) {
        (3, 2) if args.name_is("GET") => CommandRef::Get(args.key_ref(1)?),
        (3, 3) if args.name_is("SET") => CommandRef::Set(args.key_ref(1)?, args.get(2)?),
        (4, 2) if args.name_is("INCR") => CommandRef::Incr(args.key_ref(1)?),
        (4, 2) if args.name_is("DECR") => CommandRef::Decr(args.key_ref(1)?),
        // HGET does not validate its key in the general path either.
        (4, 3) if args.name_is("HGET") => CommandRef::HGet(args.text(1)?, args.text(2)?),
        (4, _) if n >= 4 && (n - 2).is_multiple_of(2) && args.name_is("HSET") => {
            let key = args.key_ref(1)?;
            let mut pairs = SmallVec::with_capacity((n - 2) / 2);
            for i in (2..n).step_by(2) {
                pairs.push((args.text(i)?, args.get(i + 1)?));
            }
            CommandRef::HSet(key, pairs)
        }
        (4, _) if n >= 2 && args.name_is("MGET") => CommandRef::MGet(
            (1..n)
                .map(|i| args.key_ref(i))
                .collect::<Option<SmallVec<_>>>()?,
        ),
        // 6O-B built these owned from the read buffer (`build`); 6O-C keeps
        // them borrowed, so a no-persistence push or add copies each value
        // once, into the store, and nothing else. Same checks as before: the
        // key passes `key_ref` (text, `validate_key`), list values are
        // payloads (any bytes, `is_payload_index`), set members are
        // identifiers and must be text. A frame failing any of them goes to
        // the general path for its error.
        (5, _) if n >= 3 && args.name_is("LPUSH") => {
            CommandRef::LPush(args.key_ref(1)?, payloads(args, 2)?)
        }
        (5, _) if n >= 3 && args.name_is("RPUSH") => {
            CommandRef::RPush(args.key_ref(1)?, payloads(args, 2)?)
        }
        (4, _) if n >= 3 && args.name_is("SADD") => {
            CommandRef::SAdd(args.key_ref(1)?, identifiers(args, 2)?)
        }
        _ => return None,
    })
}

/// Payload arguments from `from` on, borrowed. Sized up front: collecting
/// an `Option` iterator would grow the vector by doubling past 4.
fn payloads<'a>(args: Args<'a>, from: usize) -> Option<SmallVec<[&'a [u8]; 4]>> {
    let mut out = SmallVec::with_capacity(args.len().saturating_sub(from));
    for i in from..args.len() {
        out.push(args.get(i)?);
    }
    Some(out)
}

/// Identifier arguments from `from` on, borrowed: text, else `None`.
fn identifiers<'a>(args: Args<'a>, from: usize) -> Option<SmallVec<[&'a str; 4]>> {
    let mut out = SmallVec::with_capacity(args.len().saturating_sub(from));
    for i in from..args.len() {
        out.push(args.text(i)?);
    }
    Some(out)
}

/// The owned shapes not covered by [`build_ref`] — whose arguments the
/// store takes ownership of anyway, or which are rare — plus, for the
/// envelope's inner command, every borrowed shape turned owned.
fn build(args: Args<'_>) -> Option<Result<Command, String>> {
    if let Some(command) = build_ref(args) {
        return Some(Ok(command.to_command()));
    }
    let name = args.get(0)?;
    let n = args.len();
    let command = match (name.len(), n) {
        (3, _) if n >= 2 && args.name_is("DEL") => Command::Del(keys(args, 1)?),
        (4, _) if n >= 3 && (n - 1).is_multiple_of(2) && args.name_is("MSET") => {
            let mut pairs = Vec::with_capacity((n - 1) / 2);
            for i in (1..n).step_by(2) {
                pairs.push((args.key(i)?, args.bytes(i + 1)?));
            }
            Command::MSet(pairs)
        }
        (4, 1) if args.name_is("PING") => Command::Ping(None),
        // LPUSH, RPUSH and SADD (6O-B: built here, owned) are borrowed
        // shapes since 6O-C, so `build_ref` above returns them.
        (5, _) if n >= 4 && args.name_is("DEDUP") => return dedup(args),
        _ => return None,
    };
    Some(Ok(command))
}

fn keys(args: Args<'_>, from: usize) -> Option<Vec<String>> {
    (from..args.len()).map(|i| args.key(i)).collect()
}

/// `DEDUP <client> <id> <command…>` without copying the wrapped command.
///
/// The general path checks the envelope and then re-parses the wrapped
/// command from a *clone* of its elements. Here the envelope is checked the
/// same way and the wrapped command is built from its own slices, through the
/// direct builders when it is a hot shape. Any envelope problem goes to the
/// general path for its error.
fn dedup(args: Args<'_>) -> Option<Result<Command, String>> {
    let client = args.text(1)?;
    if client.is_empty() || client.len() > 64 {
        return None;
    }
    let id = u64::try_from(args.text(2)?.parse::<i64>().ok()?).ok()?;
    let inner = args.sub(3);
    let command = match build(inner).unwrap_or_else(|| inner.general()) {
        Ok(Command::Dedup(..)) => Err("ERR DEDUP cannot be nested".to_string()),
        Ok(command) => Ok(Command::Dedup(client.to_owned(), id, Box::new(command))),
        Err(e) => Err(e),
    };
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(args: &[&[u8]]) -> Vec<u8> {
        let mut out = format!("*{}\r\n", args.len()).into_bytes();
        for a in args {
            out.extend_from_slice(format!("${}\r\n", a.len()).as_bytes());
            out.extend_from_slice(a);
            out.extend_from_slice(b"\r\n");
        }
        out
    }

    /// What the server did before the decoder existed.
    fn general(buf: &[u8]) -> Result<Decoded, ParseError> {
        Value::frame_len(buf)?;
        let (value, consumed) = Value::parse(buf)?;
        Ok((Command::from_value(value), consumed))
    }

    fn assert_same(buf: &[u8]) {
        let mut decoder = RequestDecoder::new();
        assert_eq!(
            decoder.decode(buf),
            general(buf),
            "input {:?}",
            String::from_utf8_lossy(buf)
        );
    }

    /// Every hot shape, every arity around it, and every argument variant
    /// that could make the direct builder and the general path disagree.
    #[test]
    fn decoder_matches_the_general_path() {
        let names: &[&[u8]] = &[
            b"GET",
            b"get",
            b"Get",
            b"SET",
            b"set",
            b"INCR",
            b"incr",
            b"DECR",
            b"HGET",
            b"hget",
            b"HSET",
            b"hset",
            b"MGET",
            b"mget",
            b"MSET",
            b"mset",
            b"DEL",
            b"del",
            b"PING",
            b"ping",
            b"HMSET",
            b"LPUSH",
            b"lpush",
            b"LPush",
            b"RPUSH",
            b"rpush",
            b"SADD",
            b"sadd",
            b"LPUSHX",
            b"LPUS",
            b"SADDX",
            b"SREM",
            b"\xC5\xBFadd",
            b"lpu\xC5\xBFh",
            b"EXISTS",
            b"GETX",
            b"SE",
            b"\xC5\xBFet",
            b"INCRBY",
        ];
        let long_key = vec![b'k'; 512 * 1024 + 1];
        let variants: &[&[u8]] = &[
            b"k",
            b"key:000001",
            b"",
            b"\xff\xfe",
            b"caf\xC3\xA9",
            b"v\r\nalue",
            b"10",
            b"-3",
            b"DEDUP",
            &long_key,
        ];
        for name in names {
            for arity in 0..=5usize {
                // A sample of argument combinations: each variant in each
                // position, the rest a plain key.
                for v in variants {
                    for pos in 0..arity {
                        let mut parts: Vec<&[u8]> = vec![name];
                        for i in 0..arity {
                            parts.push(if i == pos { v } else { b"key" });
                        }
                        assert_same(&frame(&parts));
                    }
                }
                let mut parts: Vec<&[u8]> = vec![name];
                parts.extend(std::iter::repeat_n(b"key".as_slice(), arity));
                assert_same(&frame(&parts));
            }
        }
    }

    #[test]
    fn dedup_envelopes_match_the_general_path() {
        let inners: &[&[&[u8]]] = &[
            &[b"SET", b"k", b"\xff\x00binary"],
            &[b"set", b"k", b"v", b"EX", b"10"],
            &[b"HSET", b"h", b"f", b"\xfe"],
            &[b"INCR", b"k"],
            &[b"GET"],
            &[b"DEDUP", b"c", b"1", b"SET", b"k", b"v"],
            &[b"dedup", b"c", b"1", b"SET", b"k", b"v"],
            &[b"NOPE", b"k"],
            &[b"SET", b"", b"v"],
            &[b"SET", b"\xff", b"v"],
            &[b"LPUSH", b"l", b"\xff\x00", b"b"],
            &[b"rpush", b"l", b"a"],
            &[b"LPUSH", b"\xff", b"a"],
            &[b"LPUSH", b"l"],
            &[b"SADD", b"s", b"a", b"b", b"a"],
            &[b"SADD", b"s", b"\xff"],
            &[b"sadd", b"", b"m"],
        ];
        let clients: &[&[u8]] = &[b"client-a", b"", &[b'c'; 65], b"\xff"];
        let ids: &[&[u8]] = &[b"9", b"0", b"-1", b"+7", b"x", b"99999999999999999999"];
        for inner in inners {
            for client in clients {
                for id in ids {
                    let mut parts: Vec<&[u8]> = vec![b"DEDUP", client, id];
                    parts.extend_from_slice(inner);
                    assert_same(&frame(&parts));
                }
            }
        }
        assert_same(&frame(&[b"DEDUP", b"c", b"1"]));
    }

    /// Frames the scan does not take: other RESP types, nulls, nesting,
    /// malformed headers and every truncation of a valid pipeline.
    #[test]
    fn non_bulk_and_malformed_frames_match_the_general_path() {
        let cases: &[&[u8]] = &[
            b"*0\r\n",
            b"*-1\r\n",
            b"*-2\r\n",
            b"*1\r\n$-1\r\n",
            b"*2\r\n$3\r\nGET\r\n$-1\r\n",
            b"*2\r\n+GET\r\n$1\r\nk\r\n",
            b"*2\r\n$3\r\nGET\r\n:1\r\n",
            b"*2\r\n$3\r\nGET\r\n*1\r\n$1\r\nk\r\n",
            b"*2\r\n$3\r\nGET\r\n$1\r\nkX\n",
            b"*2\r\n$3\r\nGET\r\n$x\r\nk\r\n",
            b"*2\r\n$3\r\nGET\r\n$-5\r\nk\r\n",
            b"*x\r\n",
            b"*99999999999\r\n",
            b"*1\r\n$99999999999\r\n",
            b"+PING\r\n",
            b"$4\r\nPING\r\n",
            b":1\r\n",
            b">1\r\n$1\r\nx\r\n",
            b"%1\r\n$1\r\nk\r\n$1\r\nv\r\n",
            b"?\r\n",
            b"",
            b"*1\r\n$4\r\nPING\r\n",
            b"*1\r\n$4\r\nPING\r\n*1\r\n$4\r\nPING\r\n",
            b"*+2\r\n$3\r\nGET\r\n$1\r\nk\r\n",
            b"*2\r\n$+3\r\nGET\r\n$1\r\nk\r\n",
            b"*2\r\n$03\r\nGET\r\n$1\r\nk\r\n",
        ];
        for case in cases {
            assert_same(case);
        }
        let mut pipeline = frame(&[b"SET", b"key", b"value"]);
        pipeline.extend(frame(&[b"GET", b"key"]));
        for cut in 0..=pipeline.len() {
            assert_same(&pipeline[..cut]);
        }
    }

    /// Binary values and empty arguments survive the direct builders byte
    /// for byte.
    #[test]
    fn binary_and_empty_values_are_kept_verbatim() {
        let mut d = RequestDecoder::new();
        let value: &[u8] = b"\x00\xff\r\n\xc3(";
        let (cmd, used) = d
            .decode(&frame(&[b"SET", b"k", value]))
            .expect("whole frame");
        assert_eq!(used, frame(&[b"SET", b"k", value]).len());
        assert_eq!(
            cmd,
            Ok(Command::Set(
                "k".into(),
                value.to_vec(),
                SetOptions::default()
            ))
        );
        let (cmd, _) = d.decode(&frame(&[b"SET", b"k", b""])).expect("whole frame");
        assert_eq!(
            cmd,
            Ok(Command::Set("k".into(), vec![], SetOptions::default()))
        );
    }

    /// The decoder's own buffer does not carry state from one frame to the
    /// next, whatever the previous frame was.
    #[test]
    fn decoder_reuse_carries_nothing_over() {
        let mut d = RequestDecoder::new();
        let long = frame(&[b"MSET", b"a", b"1", b"b", b"2", b"c", b"3"]);
        let short = frame(&[b"GET", b"z"]);
        assert!(d.decode(&long).is_ok());
        assert_eq!(d.decode(&short), general(&short));
        assert_eq!(d.decode(b"*1\r\n$-1\r\n"), general(b"*1\r\n$-1\r\n"));
        assert_eq!(d.decode(&short), general(&short));
    }

    /// 6O-C: LPUSH, RPUSH and SADD are decoded borrowed (not merely
    /// equivalent), and become exactly the owned command; the frames the
    /// general path must answer are not borrowed.
    #[test]
    fn collection_writes_decode_borrowed_and_become_the_owned_command() {
        let six: Vec<&[u8]> = vec![b"1", b"2", b"3", b"4", b"5", b"6"];
        let mut many: Vec<&[u8]> = vec![b"LPUSH", b"l"];
        many.extend(six.iter().copied());
        let borrowed: Vec<(Vec<&[u8]>, Command)> = vec![
            (
                vec![b"LPUSH", b"l", b"v"],
                Command::LPush("l".into(), vec![b"v".to_vec()]),
            ),
            (
                vec![b"lpush", b"l", b"a", b"b"],
                Command::LPush("l".into(), vec![b"a".to_vec(), b"b".to_vec()]),
            ),
            (
                many.clone(),
                Command::LPush("l".into(), six.iter().map(|v| v.to_vec()).collect()),
            ),
            (
                vec![b"RPUSH", b"l", b"", b"\xff\x00\r\n"],
                Command::RPush("l".into(), vec![Vec::new(), b"\xff\x00\r\n".to_vec()]),
            ),
            (
                vec![b"SADD", b"s", b"a", b"a", b"caf\xC3\xA9"],
                Command::SAdd("s".into(), vec!["a".into(), "a".into(), "café".into()]),
            ),
            (
                vec![b"Sadd", b"s", b""],
                Command::SAdd("s".into(), vec![String::new()]),
            ),
        ];
        let mut d = RequestDecoder::new();
        for (parts, want) in &borrowed {
            let buf = frame(parts);
            match d.decode_any(&buf) {
                DecodedAny::Borrowed(command, consumed) => {
                    assert_eq!(consumed, buf.len());
                    assert!(command.is_write());
                    assert_eq!(
                        command.write_key(),
                        Some(if parts[1] == b"l" { "l" } else { "s" })
                    );
                    assert_eq!(&command.to_command(), want, "{parts:?}");
                }
                DecodedAny::Owned(other) => panic!("{parts:?} not borrowed: {other:?}"),
            }
            assert_same(&buf);
        }
        let general_only: Vec<Vec<&[u8]>> = vec![
            vec![b"LPUSH", b"l"],
            vec![b"SADD", b"s"],
            vec![b"LPUSH", b"\xff", b"v"],
            vec![b"LPUSH", b"", b"v"],
            vec![b"SADD", b"s", b"ok", b"\xff"],
            vec![b"\xC5\xBFadd", b"s", b"m"],
            vec![b"lpu\xC5\xBFh", b"l", b"v"],
            vec![b"LPUSHX", b"l", b"v"],
        ];
        for parts in &general_only {
            let buf = frame(parts);
            assert!(
                !matches!(d.decode_any(&buf), DecodedAny::Borrowed(..)),
                "{parts:?} must go to the general path"
            );
            assert_same(&buf);
        }
    }

    /// A collection write split across reads: every prefix is what the
    /// general path says (incomplete, with its bound), and the whole frame
    /// is borrowed.
    #[test]
    fn collection_writes_split_anywhere_match_the_general_path() {
        for parts in [
            vec![b"LPUSH".as_slice(), b"list", b"value-1", b"value-2"],
            vec![b"RPUSH", b"list", b"\r\n"],
            vec![b"SADD", b"set", b"member", b"member"],
        ] {
            let buf = frame(&parts);
            for cut in 0..buf.len() {
                assert_same(&buf[..cut]);
            }
            let mut d = RequestDecoder::new();
            assert!(matches!(d.decode_any(&buf), DecodedAny::Borrowed(..)));
        }
    }
}
