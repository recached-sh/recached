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
        if let Some(consumed) = self.scan(buf) {
            let args = Args {
                buf,
                spans: &self.spans,
            };
            let command = build(args).unwrap_or_else(|| args.general());
            return Ok((command, consumed));
        }
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
        let key = self.text(i)?;
        validate_key(key).ok()?;
        Some(key.to_owned())
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
fn build(args: Args<'_>) -> Option<Result<Command, String>> {
    let name = args.get(0)?;
    let n = args.len();
    // Dispatch on length first: one integer compare rejects most names
    // before any byte comparison.
    let command = match (name.len(), n) {
        (3, 2) if args.name_is("GET") => Command::Get(args.key(1)?),
        (3, 3) if args.name_is("SET") => {
            Command::Set(args.key(1)?, args.bytes(2)?, SetOptions::default())
        }
        (4, 2) if args.name_is("INCR") => Command::Incr(args.key(1)?),
        (4, 2) if args.name_is("DECR") => Command::Decr(args.key(1)?),
        (4, 3) if args.name_is("HGET") => {
            // HGET does not validate its key in the general path either.
            Command::HGet(args.text(1)?.to_owned(), args.text(2)?.to_owned())
        }
        (4, _) if n >= 4 && (n - 2).is_multiple_of(2) && args.name_is("HSET") => {
            let key = args.key(1)?;
            let mut pairs = Vec::with_capacity((n - 2) / 2);
            for i in (2..n).step_by(2) {
                pairs.push((args.text(i)?.to_owned(), args.bytes(i + 1)?));
            }
            Command::HSet(key, pairs)
        }
        (4, _) if n >= 2 && args.name_is("MGET") => Command::MGet(keys(args, 1)?),
        (3, _) if n >= 2 && args.name_is("DEL") => Command::Del(keys(args, 1)?),
        (4, _) if n >= 3 && (n - 1).is_multiple_of(2) && args.name_is("MSET") => {
            let mut pairs = Vec::with_capacity((n - 1) / 2);
            for i in (1..n).step_by(2) {
                pairs.push((args.key(i)?, args.bytes(i + 1)?));
            }
            Command::MSet(pairs)
        }
        (4, 1) if args.name_is("PING") => Command::Ping(None),
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
}
