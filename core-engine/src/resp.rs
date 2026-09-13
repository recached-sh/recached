const MAX_ARRAY_DEPTH: usize = 16;
/// Most elements one aggregate reply may contain.
///
/// Public because it bounds replies the server *builds* as well as frames it
/// parses: a command that can generate elements without an underlying
/// collection to clamp it (`SRANDMEMBER key -<n>`) has to refuse anything a
/// client could not parse back.
pub const MAX_ARRAY_ELEMENTS: usize = 1_000_000;
/// Elements reserved up front for an aggregate, however many its header claims.
///
/// The header is attacker-controlled and arrives before any of the elements do,
/// so reserving `count` capacity let a tiny input demand a large allocation:
/// `*1000000\r\n` is nine bytes and used to reserve a million `Value`s, and
/// nesting that to `MAX_ARRAY_DEPTH` multiplied it. Worse, an incomplete frame
/// is re-parsed from the start each time more bytes arrive, so a one-byte-per-
/// packet drip repeated the reservation on every packet — none of it counted
/// against `RECACHED_MAX_MEMORY`, which only tracks stored data.
///
/// Reserving a fixed floor instead makes allocation proportional to bytes
/// *received* rather than bytes *claimed*; the vector still grows as real
/// elements are parsed, and `MAX_ARRAY_ELEMENTS` still rejects absurd headers.
const PREALLOC_ELEMENTS: usize = 1024;

/// Capacity to reserve for an aggregate whose header declares `declared`.
fn prealloc_for(declared: usize) -> usize {
    declared.min(PREALLOC_ELEMENTS)
}
/// Largest bulk string the parser will accept, in bytes. Public because
/// `CONFIG GET proto-max-bulk-len` has to report the limit actually enforced
/// rather than a second copy of the number.
pub const MAX_BULK_STRING_BYTES: usize = 64 * 1024 * 1024; // 64 MB
const MAX_TOTAL_MESSAGE_BYTES: usize = 64 * 1024 * 1024; // 64 MB total per message

/// Bytes in the shortest encodable RESP value — `+\r\n`, the empty simple
/// string. Used to bound from below how much space an aggregate's outstanding
/// elements must still take up, which is what makes `ParseError::Incomplete`'s
/// `needed` useful for a multi-element frame rather than just the element it
/// happens to be blocked on.
pub const MIN_ELEMENT_BYTES: usize = 3;

/// Why a buffer could not be parsed as a RESP frame.
///
/// [`ParseError::Incomplete`] is not really a failure: it is the ordinary
/// answer for a frame whose bytes have not all arrived, which in a streaming
/// server is most reads. It carries no payload precisely so that the common
/// case allocates nothing.
///
/// This replaces a `Result<_, String>` whose callers told the two apart by
/// comparing the message to the literal `"Incomplete"` — so a partial TCP
/// segment cost a heap allocation, and a typo in that literal would silently
/// reclassify a half-arrived frame as a protocol violation and reset the
/// connection.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    /// The buffer does not yet hold a complete frame; read more and retry.
    ///
    /// `needed` is a *lower bound* on the total bytes this frame will occupy,
    /// counted from the start of the slice that was handed to [`Value::parse`].
    /// A caller that re-parses only once the buffer has grown to `needed` skips
    /// the re-parses that cannot possibly succeed.
    ///
    /// It is a lower bound and never an over-estimate — over-estimating would
    /// stall a connection forever, waiting on bytes the client already sent.
    /// Where the exact length is knowable (a bulk string whose header has
    /// arrived) it is exact; for an aggregate it is the bytes consumed so far
    /// plus [`MIN_ELEMENT_BYTES`] for every element still to come.
    Incomplete { needed: usize },
    /// The leading byte is not one of `+ - : $ * > %`.
    InvalidType(u8),
    /// A length or count header was not a well-formed number.
    InvalidHeader(&'static str),
    /// A declared length or element count is over its limit.
    TooLarge {
        what: &'static str,
        declared: u64,
        max: usize,
        unit: &'static str,
    },
    /// Aggregates nested deeper than `MAX_ARRAY_DEPTH`.
    DepthExceeded,
    /// The frame grew past `MAX_TOTAL_MESSAGE_BYTES` while being parsed.
    MessageTooLarge,
}

impl ParseError {
    /// True when the frame is merely unfinished, so the caller should wait for
    /// more bytes instead of treating this as a protocol violation.
    #[must_use]
    pub fn is_incomplete(&self) -> bool {
        matches!(self, ParseError::Incomplete { .. })
    }

    /// The lower bound carried by [`ParseError::Incomplete`], or 0 for any
    /// other variant — a caller gating on it wants "parse now" for real errors.
    #[must_use]
    pub fn needed(&self) -> usize {
        match self {
            ParseError::Incomplete { needed } => *needed,
            _ => 0,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Incomplete { .. } => f.write_str("Incomplete"),
            ParseError::InvalidType(b) => write!(f, "Invalid RESP type (byte {b:#04x})"),
            ParseError::InvalidHeader(what) => f.write_str(what),
            ParseError::TooLarge {
                what,
                declared,
                max,
                unit,
            } => write!(f, "ERR {what} too large ({declared} > {max} {unit})"),
            ParseError::DepthExceeded => f.write_str("ERR max nesting depth exceeded"),
            ParseError::MessageTooLarge => f.write_str("ERR message too large"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),
    Array(Option<Vec<Value>>),
    /// RESP3 Push frame (`>N\r\n...`). Used for server-initiated out-of-band messages
    /// (mutation fan-out, pub/sub) on the WebSocket channel. Never sent as a command response.
    Push(Vec<Value>),
    /// RESP3 Map (`%N\r\n` followed by `N` key/value pairs).
    ///
    /// Only `HELLO 3` replies with one today. A RESP2 connection must never be
    /// sent a map — the type does not exist in RESP2 and the client will fail
    /// to parse it — so the caller picks the shape from the negotiated version.
    Map(Vec<(Value, Value)>),
}

impl Value {
    /// Serializes the Value back into RESP format.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize_into(&mut buf);
        buf
    }

    /// Appends the RESP encoding of `self` to `out`. Unlike `serialize`, this
    /// allocates nothing of its own — array elements are encoded in place —
    /// so hot paths can reuse one output buffer across responses.
    pub fn serialize_into(&self, out: &mut Vec<u8>) {
        use std::io::Write;
        match self {
            Value::SimpleString(s) => {
                out.push(b'+');
                out.extend_from_slice(s.as_bytes());
                out.extend_from_slice(b"\r\n");
            }
            Value::Error(s) => {
                out.push(b'-');
                out.extend_from_slice(s.as_bytes());
                out.extend_from_slice(b"\r\n");
            }
            Value::Integer(i) => {
                let _ = write!(out, ":{}\r\n", i);
            }
            Value::BulkString(None) => {
                out.extend_from_slice(b"$-1\r\n");
            }
            Value::BulkString(Some(data)) => {
                let _ = write!(out, "${}\r\n", data.len());
                out.extend_from_slice(data);
                out.extend_from_slice(b"\r\n");
            }
            Value::Array(None) => {
                out.extend_from_slice(b"*-1\r\n");
            }
            Value::Array(Some(arr)) => {
                let _ = write!(out, "*{}\r\n", arr.len());
                for v in arr {
                    v.serialize_into(out);
                }
            }
            Value::Push(arr) => {
                let _ = write!(out, ">{}\r\n", arr.len());
                for v in arr {
                    v.serialize_into(out);
                }
            }
            Value::Map(pairs) => {
                // The header counts *pairs*, not elements, so a 3-entry map is
                // `%3` followed by six values.
                let _ = write!(out, "%{}\r\n", pairs.len());
                for (k, v) in pairs {
                    k.serialize_into(out);
                    v.serialize_into(out);
                }
            }
        }
    }

    /// Parses a byte slice into a RESP Value, returning the Value and the number of bytes consumed.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::Incomplete`] when `buffer` holds only part of a
    /// frame — the caller should read more bytes and call again with the same
    /// starting offset. Any other variant is a protocol violation and the
    /// connection should be reset.
    pub fn parse(buffer: &[u8]) -> Result<(Value, usize), ParseError> {
        match Self::parse_inner(buffer, 0, true)? {
            (Some(v), n) => Ok((v, n)),
            // Unreachable with `build = true`; answered rather than asserted
            // because nothing in this module panics on input.
            (None, _) => Err(ParseError::InvalidHeader("internal: frame not built")),
        }
    }

    /// Measures the frame at the start of `buffer` without building it.
    ///
    /// Same walk, same validation and the same limits as [`Value::parse`] —
    /// literally the same code, with value construction switched off — so the
    /// two can never disagree about what counts as a complete frame. What it
    /// skips is the allocation: bulk payloads are stepped over by arithmetic
    /// instead of being copied into a `Vec`.
    ///
    /// This exists for the streaming read path. `parse` restarts from the first
    /// byte on every call, so a large multi-bulk arriving over hundreds of TCP
    /// segments rebuilt — and reallocated — every element it had already seen,
    /// once per segment, throwing all of it away each time. Checking
    /// completeness with `frame_len` first turns that into an integer walk over
    /// headers, and `parse` then runs exactly once, on a frame known to be whole.
    ///
    /// # Errors
    ///
    /// Identical to [`Value::parse`].
    pub fn frame_len(buffer: &[u8]) -> Result<usize, ParseError> {
        Self::parse_inner(buffer, 0, false).map(|(_, n)| n)
    }

    /// `build == false` measures and validates without constructing values.
    fn parse_inner(
        buffer: &[u8],
        depth: usize,
        build: bool,
    ) -> Result<(Option<Value>, usize), ParseError> {
        let Some(&tag) = buffer.first() else {
            return Err(ParseError::Incomplete { needed: 1 });
        };
        match tag {
            b'+' => Self::parse_simple_string(buffer, build),
            b'-' => Self::parse_error(buffer, build),
            b':' => Self::parse_integer(buffer, build),
            b'$' => Self::parse_bulk_string(buffer, build),
            b'*' => Self::parse_array(buffer, depth, build),
            b'>' => Self::parse_push(buffer, depth, build),
            b'%' => Self::parse_map(buffer, depth, build),
            other => Err(ParseError::InvalidType(other)),
        }
    }

    fn read_until_crlf(buffer: &[u8]) -> Option<(&[u8], usize)> {
        // Searches for CR and checks the byte behind it, rather than testing
        // every adjacent pair: one comparison per byte instead of two, and
        // `position` drops the bounds checks the indexed loop carried.
        //
        // The scan semantics are deliberately unchanged — still the *first*
        // CR immediately followed by LF. Stopping at the first LF instead
        // would be faster again, but a header containing a bare LF would then
        // report `Incomplete` rather than `InvalidHeader`, telling the caller
        // to wait for more bytes on a frame that is already malformed.
        let mut from = 0;
        while let Some(offset) = buffer[from..].iter().position(|&b| b == b'\r') {
            let cr = from + offset;
            match buffer.get(cr + 1) {
                Some(b'\n') => return Some((&buffer[1..cr], cr + 2)),
                Some(_) => from = cr + 1,
                None => return None,
            }
        }
        None
    }

    /// Parse a RESP header integer straight from its bytes.
    ///
    /// The five header sites used to run `String::from_utf8_lossy(data)` and
    /// then the generic `str::parse`, which validates UTF-8 across the header
    /// and walks `FromStr` for what is almost always one to three ASCII
    /// digits. Parsing was ~8% of a `SET` on the profile.
    ///
    /// Accepts exactly what `i64::from_str` accepts — an optional `+` or `-`
    /// followed by at least one ASCII digit, nothing else — and accumulates
    /// negatives downward so `i64::MIN` still parses rather than overflowing.
    fn parse_header_int(data: &[u8]) -> Option<i64> {
        let (negative, digits) = match data.first()? {
            b'-' => (true, &data[1..]),
            b'+' => (false, &data[1..]),
            _ => (false, data),
        };
        if digits.is_empty() {
            return None;
        }
        let mut acc: i64 = 0;
        for &byte in digits {
            if !byte.is_ascii_digit() {
                return None;
            }
            let digit = (byte - b'0') as i64;
            acc = acc.checked_mul(10)?;
            acc = if negative {
                acc.checked_sub(digit)?
            } else {
                acc.checked_add(digit)?
            };
        }
        Some(acc)
    }

    /// The unsigned form, for the headers that were parsed as `u64`.
    ///
    /// Separate from [`parse_header_int`] rather than a signed parse plus a
    /// sign check, because those headers must reject a negative outright: a
    /// `>-1\r\n` push frame parsed as `-1` would sail past an upper-bound
    /// check that only looks for values that are too large.
    fn parse_header_uint(data: &[u8]) -> Option<u64> {
        let digits = match data.first()? {
            b'+' => &data[1..],
            b'-' => return None,
            _ => data,
        };
        if digits.is_empty() {
            return None;
        }
        let mut acc: u64 = 0;
        for &byte in digits {
            if !byte.is_ascii_digit() {
                return None;
            }
            acc = acc.checked_mul(10)?.checked_add((byte - b'0') as u64)?;
        }
        Some(acc)
    }

    /// The `Incomplete` bound for a header whose terminating CRLF has not
    /// arrived: whatever we hold, plus at least one more byte.
    fn need_more(buffer: &[u8]) -> ParseError {
        ParseError::Incomplete {
            needed: buffer.len() + 1,
        }
    }

    /// Re-bases a child's `Incomplete` bound onto the enclosing aggregate.
    ///
    /// `consumed` is what the aggregate has already eaten — its header plus
    /// every element that finished — and `outstanding` is how many elements
    /// have not been started at all. Each of those must still occupy at least
    /// [`MIN_ELEMENT_BYTES`], so charging for them is what makes the bound
    /// useful for a large multi-bulk: blocked on element 3 of 20 000, the
    /// caller learns it needs roughly 60 KB more, not "one more byte".
    ///
    /// Only `Incomplete` is re-based; a real protocol error propagates as-is.
    fn widen(err: ParseError, consumed: usize, outstanding: usize) -> ParseError {
        match err {
            ParseError::Incomplete { needed } => ParseError::Incomplete {
                needed: consumed
                    .saturating_add(needed)
                    .saturating_add(outstanding.saturating_mul(MIN_ELEMENT_BYTES)),
            },
            other => other,
        }
    }

    fn parse_simple_string(
        buffer: &[u8],
        build: bool,
    ) -> Result<(Option<Value>, usize), ParseError> {
        match Self::read_until_crlf(buffer) {
            Some((data, len)) => Ok((
                build.then(|| Value::SimpleString(String::from_utf8_lossy(data).into_owned())),
                len,
            )),
            None => Err(Self::need_more(buffer)),
        }
    }

    fn parse_error(buffer: &[u8], build: bool) -> Result<(Option<Value>, usize), ParseError> {
        match Self::read_until_crlf(buffer) {
            Some((data, len)) => Ok((
                build.then(|| Value::Error(String::from_utf8_lossy(data).into_owned())),
                len,
            )),
            None => Err(Self::need_more(buffer)),
        }
    }

    fn parse_integer(buffer: &[u8], build: bool) -> Result<(Option<Value>, usize), ParseError> {
        match Self::read_until_crlf(buffer) {
            Some((data, len)) => {
                // Validated even when only measuring, so `frame_len` rejects
                // exactly what a build rejects.
                let i = Self::parse_header_int(data)
                    .ok_or(ParseError::InvalidHeader("Invalid integer format"))?;
                Ok((build.then_some(Value::Integer(i)), len))
            }
            None => Err(Self::need_more(buffer)),
        }
    }

    fn parse_bulk_string(buffer: &[u8], build: bool) -> Result<(Option<Value>, usize), ParseError> {
        const BAD_LEN: ParseError = ParseError::InvalidHeader("Invalid bulk string length");
        match Self::read_until_crlf(buffer) {
            Some((data, head_len)) => {
                let length = Self::parse_header_int(data).ok_or(BAD_LEN)?;

                if length == -1 {
                    return Ok((build.then_some(Value::BulkString(None)), head_len));
                }
                if length < 0 {
                    return Err(BAD_LEN);
                }

                // Compared as u64 before narrowing: on a 32-bit target (wasm32
                // builds this crate) `as usize` would truncate a declared
                // length above 4 GiB and let it past the limit check.
                let declared = length as u64;
                if declared > MAX_BULK_STRING_BYTES as u64 {
                    return Err(ParseError::TooLarge {
                        what: "bulk string",
                        declared,
                        max: MAX_BULK_STRING_BYTES,
                        unit: "bytes",
                    });
                }
                let length = declared as usize;
                let end = head_len + length + 2; // +2 for trailing CRLF
                if buffer.len() < end {
                    // The header gives the exact size, so the caller can wait
                    // for the whole payload instead of re-parsing per packet.
                    return Err(ParseError::Incomplete { needed: end });
                }

                if &buffer[head_len + length..end] != b"\r\n" {
                    return Err(ParseError::InvalidHeader(
                        "bulk string payload is not terminated by CRLF",
                    ));
                }

                // The payload is copied only when a value is being built;
                // measuring steps over it.
                let payload = build.then(|| buffer[head_len..head_len + length].to_vec());
                Ok((payload.map(|d| Value::BulkString(Some(d))), end))
            }
            None => Err(Self::need_more(buffer)),
        }
    }

    fn parse_push(
        buffer: &[u8],
        depth: usize,
        build: bool,
    ) -> Result<(Option<Value>, usize), ParseError> {
        if depth >= MAX_ARRAY_DEPTH {
            return Err(ParseError::DepthExceeded);
        }
        match Self::read_until_crlf(buffer) {
            Some((data, mut offset)) => {
                let count = Self::parse_header_uint(data)
                    .ok_or(ParseError::InvalidHeader("Invalid push frame length"))?;
                // Compared as u64: see `parse_bulk_string`.
                if count > MAX_ARRAY_ELEMENTS as u64 {
                    return Err(ParseError::TooLarge {
                        what: "push frame",
                        declared: count,
                        max: MAX_ARRAY_ELEMENTS,
                        unit: "elements",
                    });
                }
                let count = count as usize;
                let mut arr = Self::element_buffer(count, build);
                for i in 0..count {
                    let (val, len) = Self::parse_inner(&buffer[offset..], depth + 1, build)
                        .map_err(|e| Self::widen(e, offset, count - i - 1))?;
                    arr.extend(val);
                    offset += len;
                    if offset > MAX_TOTAL_MESSAGE_BYTES {
                        return Err(ParseError::MessageTooLarge);
                    }
                }
                Ok((build.then_some(Value::Push(arr)), offset))
            }
            None => Err(Self::need_more(buffer)),
        }
    }

    fn parse_map(
        buffer: &[u8],
        depth: usize,
        build: bool,
    ) -> Result<(Option<Value>, usize), ParseError> {
        if depth >= MAX_ARRAY_DEPTH {
            return Err(ParseError::DepthExceeded);
        }
        match Self::read_until_crlf(buffer) {
            Some((data, mut offset)) => {
                let count = Self::parse_header_uint(data)
                    .ok_or(ParseError::InvalidHeader("Invalid map length"))?;
                // Each pair is two values, so the element budget is halved.
                const MAX_PAIRS: usize = MAX_ARRAY_ELEMENTS / 2;
                if count > MAX_PAIRS as u64 {
                    return Err(ParseError::TooLarge {
                        what: "map",
                        declared: count,
                        max: MAX_PAIRS,
                        unit: "pairs",
                    });
                }
                let count = count as usize;
                let mut pairs: Vec<(Value, Value)> = if build {
                    Vec::with_capacity(prealloc_for(count))
                } else {
                    Vec::new()
                };
                for i in 0..count {
                    // Outstanding values, not pairs: blocked on a key, this
                    // pair's value is still to come as well.
                    let after = (count - i - 1) * 2;
                    let (k, klen) = Self::parse_inner(&buffer[offset..], depth + 1, build)
                        .map_err(|e| Self::widen(e, offset, after + 1))?;
                    offset += klen;
                    let (v, vlen) = Self::parse_inner(&buffer[offset..], depth + 1, build)
                        .map_err(|e| Self::widen(e, offset, after))?;
                    offset += vlen;
                    if let (Some(k), Some(v)) = (k, v) {
                        pairs.push((k, v));
                    }
                    if offset > MAX_TOTAL_MESSAGE_BYTES {
                        return Err(ParseError::MessageTooLarge);
                    }
                }
                Ok((build.then_some(Value::Map(pairs)), offset))
            }
            None => Err(Self::need_more(buffer)),
        }
    }

    fn parse_array(
        buffer: &[u8],
        depth: usize,
        build: bool,
    ) -> Result<(Option<Value>, usize), ParseError> {
        const BAD_LEN: ParseError = ParseError::InvalidHeader("Invalid array length");
        if depth >= MAX_ARRAY_DEPTH {
            return Err(ParseError::DepthExceeded);
        }

        match Self::read_until_crlf(buffer) {
            Some((data, mut offset)) => {
                let count = Self::parse_header_int(data).ok_or(BAD_LEN)?;

                if count == -1 {
                    return Ok((build.then_some(Value::Array(None)), offset));
                }
                if count < 0 {
                    return Err(BAD_LEN);
                }
                // Compared as u64: see `parse_bulk_string`.
                let declared = count as u64;
                if declared > MAX_ARRAY_ELEMENTS as u64 {
                    return Err(ParseError::TooLarge {
                        what: "array",
                        declared,
                        max: MAX_ARRAY_ELEMENTS,
                        unit: "elements",
                    });
                }

                let count = declared as usize;
                let mut arr = Self::element_buffer(count, build);
                for i in 0..count {
                    let (val, len) = Self::parse_inner(&buffer[offset..], depth + 1, build)
                        .map_err(|e| Self::widen(e, offset, count - i - 1))?;
                    arr.extend(val);
                    offset += len;
                    if offset > MAX_TOTAL_MESSAGE_BYTES {
                        return Err(ParseError::MessageTooLarge);
                    }
                }

                Ok((build.then_some(Value::Array(Some(arr))), offset))
            }
            None => Err(Self::need_more(buffer)),
        }
    }

    /// Element accumulator for an aggregate: pre-sized when building, and left
    /// unallocated when only measuring.
    fn element_buffer(count: usize, build: bool) -> Vec<Value> {
        if build {
            Vec::with_capacity(prealloc_for(count))
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod header_parse_equivalence_tests {
    use super::*;

    /// The byte-wise header parsers must accept and reject exactly what the
    /// `String::from_utf8_lossy(..).parse()` pair they replaced did.
    ///
    /// This is a hardened parser reached by unauthenticated clients, so
    /// "faster and probably the same" is not good enough — a widened accept
    /// set here is a protocol-level hole. Compared against the originals over
    /// every shape that has ever mattered: signs, padding, overflow, empties,
    /// invalid UTF-8, and embedded NULs.
    fn reference_int(data: &[u8]) -> Option<i64> {
        String::from_utf8_lossy(data).parse::<i64>().ok()
    }

    fn reference_uint(data: &[u8]) -> Option<u64> {
        String::from_utf8_lossy(data).parse::<u64>().ok()
    }

    fn cases() -> Vec<Vec<u8>> {
        let mut out: Vec<Vec<u8>> = vec![
            b"".to_vec(),
            b"-".to_vec(),
            b"+".to_vec(),
            b"0".to_vec(),
            b"-0".to_vec(),
            b"+0".to_vec(),
            b"1".to_vec(),
            b"-1".to_vec(),
            b"+1".to_vec(),
            b"007".to_vec(),
            b"64".to_vec(),
            b"1000000".to_vec(),
            b" 1".to_vec(),
            b"1 ".to_vec(),
            b"1\t".to_vec(),
            b"1.0".to_vec(),
            b"1e3".to_vec(),
            b"--1".to_vec(),
            b"1-".to_vec(),
            b"0x10".to_vec(),
            b"1_000".to_vec(),
            b"abc".to_vec(),
            b"9223372036854775807".to_vec(),  // i64::MAX
            b"9223372036854775808".to_vec(),  // i64::MAX + 1
            b"-9223372036854775808".to_vec(), // i64::MIN
            b"-9223372036854775809".to_vec(), // i64::MIN - 1
            b"18446744073709551615".to_vec(), // u64::MAX
            b"18446744073709551616".to_vec(), // u64::MAX + 1
            b"99999999999999999999999999".to_vec(),
            vec![0xff, 0xfe],       // invalid UTF-8
            vec![b'1', 0x00, b'2'], // embedded NUL
            vec![b'1', 0xff],
        ];
        for n in [-3i64, -2, -1, 0, 1, 2, 3, 9, 10, 11, 99, 100, 12345] {
            out.push(n.to_string().into_bytes());
        }
        out
    }

    #[test]
    fn the_signed_header_parser_matches_the_std_parse_it_replaced() {
        for case in cases() {
            assert_eq!(
                Value::parse_header_int(&case),
                reference_int(&case),
                "signed mismatch on {:?}",
                String::from_utf8_lossy(&case)
            );
        }
    }

    #[test]
    fn the_unsigned_header_parser_matches_the_std_parse_it_replaced() {
        for case in cases() {
            assert_eq!(
                Value::parse_header_uint(&case),
                reference_uint(&case),
                "unsigned mismatch on {:?}",
                String::from_utf8_lossy(&case)
            );
        }
    }

    /// A negative push/map count must be refused outright rather than sliding
    /// past an upper-bound check that only looks for values that are too big.
    #[test]
    fn a_negative_push_or_map_count_is_rejected() {
        for frame in [b">-1\r\n".as_slice(), b"%-1\r\n".as_slice()] {
            let err = Value::parse(frame).expect_err("a negative count must not parse");
            assert!(
                matches!(err, ParseError::InvalidHeader(_)),
                "expected InvalidHeader for {:?}, got {err:?}",
                String::from_utf8_lossy(frame)
            );
        }
    }

    /// A bare LF inside a header must stay a protocol error. Scanning to the
    /// first LF instead of the first CRLF would report `Incomplete` and leave
    /// the connection waiting on a frame that is already malformed.
    #[test]
    fn a_bare_lf_in_a_header_is_a_protocol_error_not_incomplete() {
        let err = Value::parse(b"$5\nhello\r\n").expect_err("bare LF must not parse");
        assert!(
            !err.is_incomplete(),
            "a malformed header must not report Incomplete, got {err:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(v: &Value) {
        let bytes = v.serialize();
        let (parsed, consumed) = Value::parse(&bytes).expect("parse failed");
        assert_eq!(&parsed, v);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn simple_string_round_trip() {
        round_trip(&Value::SimpleString("PONG".to_string()));
        round_trip(&Value::SimpleString("OK".to_string()));
        round_trip(&Value::SimpleString(String::new()));
    }

    #[test]
    fn error_round_trip() {
        round_trip(&Value::Error("ERR unknown command".to_string()));
    }

    #[test]
    fn integer_round_trip() {
        round_trip(&Value::Integer(0));
        round_trip(&Value::Integer(42));
        round_trip(&Value::Integer(-1));
        round_trip(&Value::Integer(i64::MAX));
        round_trip(&Value::Integer(i64::MIN));
    }

    #[test]
    fn bulk_string_round_trip() {
        round_trip(&Value::BulkString(Some(b"hello".to_vec())));
        round_trip(&Value::BulkString(Some(b"hello world".to_vec())));
        round_trip(&Value::BulkString(Some(vec![]))); // empty bulk string
        round_trip(&Value::BulkString(None)); // null bulk string
    }

    #[test]
    fn bulk_string_with_crlf_inside() {
        // Values containing \r\n must survive round-trip via the length-prefixed format
        round_trip(&Value::BulkString(Some(b"foo\r\nbar".to_vec())));
    }

    #[test]
    fn array_round_trip() {
        round_trip(&Value::Array(None));
        round_trip(&Value::Array(Some(vec![])));
        round_trip(&Value::Array(Some(vec![
            Value::BulkString(Some(b"SET".to_vec())),
            Value::BulkString(Some(b"key".to_vec())),
            Value::BulkString(Some(b"value".to_vec())),
        ])));
    }

    #[test]
    fn nested_array_round_trip() {
        let inner = Value::Array(Some(vec![Value::Integer(1), Value::Integer(2)]));
        round_trip(&Value::Array(Some(vec![inner])));
    }

    #[test]
    fn incomplete_returns_error() {
        assert!(Value::parse(b"").unwrap_err().is_incomplete());
        assert!(Value::parse(b"+OK").unwrap_err().is_incomplete()); // missing \r\n
        assert!(Value::parse(b"$5\r\nhell").unwrap_err().is_incomplete()); // truncated bulk
        assert!(Value::parse(b"*2\r\n+OK\r\n").unwrap_err().is_incomplete()); // array missing 2nd element
    }

    #[test]
    fn invalid_resp_type_byte() {
        assert_eq!(
            Value::parse(b"!garbage"),
            Err(ParseError::InvalidType(b'!'))
        );
    }

    #[test]
    fn depth_limit_exceeded() {
        // Build a deeply nested array: *1\r\n*1\r\n... repeated MAX_ARRAY_DEPTH+1 times
        let mut payload = String::new();
        for _ in 0..=MAX_ARRAY_DEPTH {
            payload.push_str("*1\r\n");
        }
        payload.push_str("+leaf\r\n");
        let result = Value::parse(payload.as_bytes());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max nesting depth")
        );
    }

    #[test]
    fn parse_consumes_exactly_right_bytes() {
        // Two concatenated simple strings; parse should stop after the first
        let input = b"+OK\r\n+NEXT\r\n";
        let (val, consumed) = Value::parse(input).unwrap();
        assert_eq!(val, Value::SimpleString("OK".to_string()));
        assert_eq!(consumed, 5); // "+OK\r\n" = 5 bytes
    }

    #[test]
    fn push_round_trip() {
        round_trip(&Value::Map(vec![]));
        round_trip(&Value::Map(vec![(
            Value::BulkString(Some(b"proto".to_vec())),
            Value::Integer(3),
        )]));
        round_trip(&Value::Map(vec![
            (
                Value::BulkString(Some(b"server".to_vec())),
                Value::BulkString(Some(b"recached".to_vec())),
            ),
            (
                Value::BulkString(Some(b"modules".to_vec())),
                Value::Array(Some(vec![])),
            ),
        ]));
        round_trip(&Value::Push(vec![]));
        round_trip(&Value::Push(vec![
            Value::BulkString(Some(b"SET".to_vec())),
            Value::BulkString(Some(b"theme".to_vec())),
            Value::BulkString(Some(b"dark".to_vec())),
        ]));
        round_trip(&Value::Push(vec![
            Value::BulkString(Some(b"message".to_vec())),
            Value::BulkString(Some(b"alerts".to_vec())),
            Value::BulkString(Some(b"hello".to_vec())),
        ]));
    }

    #[test]
    fn map_header_counts_pairs_not_elements() {
        // `%N` means N key/value *pairs* — 2N values follow. Emitting the
        // element count instead would desynchronise every downstream parser.
        let m = Value::Map(vec![
            (Value::BulkString(Some(b"a".to_vec())), Value::Integer(1)),
            (Value::BulkString(Some(b"b".to_vec())), Value::Integer(2)),
        ]);
        let bytes = m.serialize();
        assert!(
            bytes.starts_with(b"%2\r\n"),
            "got {:?}",
            String::from_utf8_lossy(&bytes)
        );
        let (parsed, n) = Value::parse(&bytes).unwrap();
        assert_eq!(parsed, m);
        assert_eq!(n, bytes.len(), "must consume exactly the frame");
    }

    #[test]
    fn map_rejects_a_malformed_length() {
        assert!(Value::parse(b"%x\r\n").is_err());
        assert!(Value::parse(b"%-1\r\n").is_err());
    }

    #[test]
    fn truncated_map_is_incomplete_not_an_error() {
        // A partial frame must be retried once more bytes arrive, not rejected.
        let full = Value::Map(vec![(
            Value::BulkString(Some(b"k".to_vec())),
            Value::Integer(7),
        )])
        .serialize();
        for cut in 1..full.len() {
            assert!(
                Value::parse(&full[..cut]).unwrap_err().is_incomplete(),
                "prefix of length {cut} should be incomplete"
            );
        }
    }

    #[test]
    fn push_prefix_distinct_from_array() {
        let push = Value::Push(vec![Value::BulkString(Some(b"x".to_vec()))]).serialize();
        let arr = Value::Array(Some(vec![Value::BulkString(Some(b"x".to_vec()))])).serialize();
        assert_ne!(push, arr);
        assert!(push.starts_with(b">"));
        assert!(arr.starts_with(b"*"));
    }

    #[test]
    fn null_array_vs_empty_array() {
        let null = Value::Array(None).serialize();
        let empty = Value::Array(Some(vec![])).serialize();
        assert_ne!(null, empty);
        assert_eq!(null, b"*-1\r\n");
        assert_eq!(empty, b"*0\r\n");
    }
}

#[cfg(test)]
mod parser_edge_tests {
    use super::*;

    /// Every prefix of a valid frame must report `Incomplete` rather than
    /// parsing garbage — this is the property that makes streaming reads safe
    /// when TCP hands us a partial frame.
    fn every_prefix_is_incomplete(frame: &[u8]) {
        for cut in 1..frame.len() {
            let err = Value::parse(&frame[..cut]).expect_err(&format!(
                "prefix of {cut} bytes should not parse: {frame:?}"
            ));
            assert!(
                matches!(
                    err,
                    ParseError::Incomplete { .. }
                        | ParseError::TooLarge { .. }
                        | ParseError::InvalidType(_)
                        | ParseError::InvalidHeader(_)
                ),
                "prefix {cut}: unexpected error {err}"
            );
        }
    }

    #[test]
    fn parses_error_frames() {
        let (v, n) = Value::parse(b"-ERR something broke\r\n").unwrap();
        assert_eq!(v, Value::Error("ERR something broke".to_string()));
        assert_eq!(n, 22);
    }

    #[test]
    fn parses_integer_frames_including_negatives() {
        assert_eq!(Value::parse(b":42\r\n").unwrap().0, Value::Integer(42));
        assert_eq!(Value::parse(b":-7\r\n").unwrap().0, Value::Integer(-7));
        assert_eq!(Value::parse(b":0\r\n").unwrap().0, Value::Integer(0));
    }

    #[test]
    fn rejects_malformed_integer() {
        let err = Value::parse(b":notanumber\r\n").unwrap_err().to_string();
        assert!(err.contains("Invalid integer"), "got {err}");
    }

    #[test]
    fn rejects_bulk_payload_without_trailing_crlf() {
        let err = Value::parse(b"$3\r\nabcXX").unwrap_err().to_string();
        assert!(err.contains("terminated by CRLF"), "got {err}");
    }

    #[test]
    fn partial_frames_report_incomplete() {
        every_prefix_is_incomplete(b"-ERR broke\r\n");
        every_prefix_is_incomplete(b":123\r\n");
        every_prefix_is_incomplete(b"$5\r\nhello\r\n");
        every_prefix_is_incomplete(b"*2\r\n$1\r\na\r\n$1\r\nb\r\n");
    }

    #[test]
    fn parses_push_frames() {
        let frame = b">2\r\n$7\r\nmessage\r\n$2\r\nhi\r\n";
        let (v, n) = Value::parse(frame).unwrap();
        assert_eq!(
            v,
            Value::Push(vec![
                Value::BulkString(Some(b"message".to_vec())),
                Value::BulkString(Some(b"hi".to_vec())),
            ])
        );
        assert_eq!(n, frame.len());
    }

    #[test]
    fn push_frame_round_trips() {
        let v = Value::Push(vec![
            Value::SimpleString("keychange".into()),
            Value::Integer(1),
        ]);
        let bytes = v.serialize();
        assert_eq!(Value::parse(&bytes).unwrap().0, v);
    }

    #[test]
    fn rejects_malformed_push_length() {
        let err = Value::parse(b">abc\r\n").unwrap_err().to_string();
        assert!(err.contains("Invalid push frame length"), "got {err}");
    }

    #[test]
    fn rejects_push_frame_with_too_many_elements() {
        // Declared count over the element cap must be refused on the header
        // alone, before any allocation proportional to it.
        let frame = format!(">{}\r\n", MAX_ARRAY_ELEMENTS + 1);
        let err = Value::parse(frame.as_bytes()).unwrap_err().to_string();
        assert!(err.contains("push frame too large"), "got {err}");
    }

    #[test]
    fn rejects_array_frame_with_too_many_elements() {
        let frame = format!("*{}\r\n", MAX_ARRAY_ELEMENTS + 1);
        let err = Value::parse(frame.as_bytes()).unwrap_err().to_string();
        assert!(err.contains("too large"), "got {err}");
    }

    #[test]
    fn rejects_nesting_beyond_the_depth_limit() {
        // A hostile client can otherwise drive unbounded recursion with a few
        // bytes per level.
        let mut frame = Vec::new();
        for _ in 0..(MAX_ARRAY_DEPTH + 2) {
            frame.extend_from_slice(b"*1\r\n");
        }
        frame.extend_from_slice(b"$1\r\na\r\n");
        let err = Value::parse(&frame).unwrap_err().to_string();
        assert!(err.contains("nesting depth"), "got {err}");
    }

    #[test]
    fn nesting_within_the_limit_still_parses() {
        let mut frame = Vec::new();
        for _ in 0..(MAX_ARRAY_DEPTH - 2) {
            frame.extend_from_slice(b"*1\r\n");
        }
        frame.extend_from_slice(b"$1\r\na\r\n");
        assert!(Value::parse(&frame).is_ok());
    }

    #[test]
    fn rejects_oversized_bulk_string_header() {
        let frame = format!("${}\r\n", MAX_BULK_STRING_BYTES + 1);
        let err = Value::parse(frame.as_bytes()).unwrap_err().to_string();
        assert!(err.contains("too large"), "got {err}");
    }

    #[test]
    fn rejects_unknown_type_byte() {
        let err = Value::parse(b"%1\r\n").unwrap_err().to_string();
        assert!(!err.is_empty());
    }

    #[test]
    fn empty_buffer_is_incomplete() {
        assert!(Value::parse(b"").is_err());
    }

    #[test]
    fn null_array_and_null_bulk_round_trip() {
        assert_eq!(Value::parse(b"*-1\r\n").unwrap().0, Value::Array(None));
        assert_eq!(Value::parse(b"$-1\r\n").unwrap().0, Value::BulkString(None));
    }

    #[test]
    fn parse_reports_bytes_consumed_so_pipelines_advance() {
        // Two frames back to back: the parser must consume exactly the first.
        let buf = b":1\r\n:2\r\n";
        let (v1, n1) = Value::parse(buf).unwrap();
        assert_eq!(v1, Value::Integer(1));
        assert_eq!(n1, 4);
        let (v2, n2) = Value::parse(&buf[n1..]).unwrap();
        assert_eq!(v2, Value::Integer(2));
        assert_eq!(n2, 4);
    }
}

// ── Parse error typing ────────────────────────────────────────────────────────

/// `Value::parse` used to return `Result<_, String>`, and the server told a
/// half-arrived frame from a protocol violation by comparing that string to the
/// literal `"Incomplete"`. These tests pin the replacement contract: the
/// distinction is a variant, the common case allocates nothing, and the wire
/// text callers log is unchanged.
#[cfg(test)]
mod parse_error_tests {
    use super::*;

    #[test]
    fn a_partial_frame_is_incomplete_not_a_protocol_error() {
        let frame = Value::Array(Some(vec![
            Value::BulkString(Some(b"SET".to_vec())),
            Value::BulkString(Some(b"k".to_vec())),
            Value::BulkString(Some(b"v".to_vec())),
        ]))
        .serialize();

        for cut in 1..frame.len() {
            let err = Value::parse(&frame[..cut]).unwrap_err();
            assert!(
                err.is_incomplete(),
                "prefix of {cut} bytes classified as {err:?}, which would reset the connection"
            );
        }
        // And the whole thing parses.
        assert!(Value::parse(&frame).is_ok());
    }

    #[test]
    fn a_real_protocol_violation_is_not_incomplete() {
        for bad in [
            &b"!garbage\r\n"[..],
            &b"*abc\r\n"[..],
            &b":notanumber\r\n"[..],
            &b"$xyz\r\n"[..],
        ] {
            let err = Value::parse(bad).unwrap_err();
            assert!(
                !err.is_incomplete(),
                "{:?} must not be mistaken for a partial frame",
                String::from_utf8_lossy(bad)
            );
        }
    }

    #[test]
    fn an_unknown_type_byte_reports_which_byte_it_was() {
        assert_eq!(
            Value::parse(b"!x\r\n").unwrap_err(),
            ParseError::InvalidType(b'!')
        );
    }

    #[test]
    fn the_wire_text_of_each_error_is_unchanged() {
        // These strings reach operators through `warn!("TCP protocol error: …")`
        // and the RESP error replies; drift makes existing runbooks wrong.
        assert_eq!(
            ParseError::Incomplete { needed: 1 }.to_string(),
            "Incomplete"
        );
        assert_eq!(
            ParseError::DepthExceeded.to_string(),
            "ERR max nesting depth exceeded"
        );
        assert_eq!(
            ParseError::MessageTooLarge.to_string(),
            "ERR message too large"
        );
        assert_eq!(
            ParseError::TooLarge {
                what: "bulk string",
                declared: 99,
                max: 10,
                unit: "bytes",
            }
            .to_string(),
            "ERR bulk string too large (99 > 10 bytes)"
        );
        assert_eq!(
            ParseError::TooLarge {
                what: "map",
                declared: 9,
                max: 4,
                unit: "pairs",
            }
            .to_string(),
            "ERR map too large (9 > 4 pairs)"
        );
    }

    /// The limits are compared in `u64` before narrowing to `usize`. On wasm32
    /// — which this crate is built for — `as usize` truncates, and a declared
    /// count just above 2^32 would wrap to a small number and slip past the
    /// check. The comparison must reject it on every target.
    #[test]
    fn a_declared_count_above_the_32_bit_range_is_still_rejected() {
        for frame in [
            format!("*{}\r\n", u32::MAX as u64 + 2),
            format!(">{}\r\n", u32::MAX as u64 + 2),
            format!("%{}\r\n", u32::MAX as u64 + 2),
            format!("${}\r\n", u32::MAX as u64 + 2),
        ] {
            let err = Value::parse(frame.as_bytes()).unwrap_err();
            assert!(
                matches!(err, ParseError::TooLarge { .. }),
                "{frame:?} gave {err:?}, expected TooLarge"
            );
        }
    }

    #[test]
    fn is_incomplete_agrees_with_the_variant() {
        assert!(ParseError::Incomplete { needed: 1 }.is_incomplete());
        assert!(!ParseError::DepthExceeded.is_incomplete());
        assert!(!ParseError::InvalidType(b'!').is_incomplete());
        assert!(!ParseError::InvalidHeader("Invalid array length").is_incomplete());
    }
}

// ── Incomplete bounds ─────────────────────────────────────────────────────────

/// `ParseError::Incomplete` carries a lower bound on the finished frame's size
/// so the server can skip re-parses that cannot succeed. Without it, a large
/// multi-bulk arriving over hundreds of TCP segments was re-parsed from byte
/// zero on every segment, reallocating every bulk string already received.
///
/// The bound being a *lower* bound is load-bearing: an over-estimate would make
/// the server wait for bytes the client already sent, hanging the connection.
#[cfg(test)]
mod incomplete_bound_tests {
    use super::*;

    fn multibulk(elements: usize, value_len: usize) -> Vec<u8> {
        Value::Array(Some(
            (0..elements)
                .map(|i| Value::BulkString(Some(vec![b'a' + (i % 26) as u8; value_len])))
                .collect(),
        ))
        .serialize()
    }

    /// The safety property, checked exhaustively over every prefix: the bound
    /// may never exceed the real frame length.
    #[test]
    fn the_bound_never_exceeds_the_real_frame_length() {
        for frame in [
            multibulk(20, 8),
            multibulk(3, 200),
            multibulk(1, 0),
            Value::Map(vec![
                (Value::BulkString(Some(b"k".to_vec())), Value::Integer(1)),
                (Value::BulkString(Some(b"kk".to_vec())), Value::Integer(2)),
            ])
            .serialize(),
            Value::Push(vec![
                Value::BulkString(Some(b"message".to_vec())),
                Value::BulkString(Some(b"chan".to_vec())),
            ])
            .serialize(),
            Value::Array(Some(vec![
                Value::Array(Some(vec![Value::Integer(1), Value::Integer(2)])),
                Value::BulkString(Some(b"nested".to_vec())),
            ]))
            .serialize(),
        ] {
            for cut in 0..frame.len() {
                let err = Value::parse(&frame[..cut]).unwrap_err();
                assert!(err.is_incomplete(), "prefix {cut} gave {err:?}");
                assert!(
                    err.needed() <= frame.len(),
                    "prefix {cut}: bound {} exceeds the {} byte frame — a server \
                     waiting for that would hang on a frame the client finished",
                    err.needed(),
                    frame.len()
                );
            }
        }
    }

    /// The bound must also be strictly ahead of what we already hold, or the
    /// gate would never let anything through and the loop would spin.
    #[test]
    fn the_bound_always_asks_for_at_least_one_more_byte() {
        let frame = multibulk(12, 16);
        for cut in 0..frame.len() {
            let err = Value::parse(&frame[..cut]).unwrap_err();
            assert!(
                err.needed() > cut,
                "prefix {cut} asked for only {} bytes",
                err.needed()
            );
        }
    }

    /// A bulk string whose header has arrived reports its exact end, so a large
    /// value is not re-parsed once per segment while its payload streams in.
    #[test]
    fn a_bulk_header_yields_the_exact_frame_length() {
        let frame = Value::BulkString(Some(vec![b'x'; 10_000])).serialize();
        // Header only: `$10000\r\n`.
        let header = frame.iter().position(|&b| b == b'\n').unwrap() + 1;
        for cut in header..frame.len() {
            assert_eq!(
                Value::parse(&frame[..cut]).unwrap_err().needed(),
                frame.len(),
                "prefix {cut} should already know the exact length"
            );
        }
    }

    /// The point of the whole change: inside a multi-bulk, the bound accounts
    /// for the elements not yet started, so it grows far beyond "one more byte"
    /// and the caller skips whole runs of segments.
    #[test]
    fn a_multibulk_bound_accounts_for_elements_not_yet_started() {
        let elements = 5_000;
        let frame = multibulk(elements, 4);
        // Just past the `*5000\r\n` header, nothing else parsed yet.
        let after_header = frame.iter().position(|&b| b == b'\n').unwrap() + 1;
        let needed = Value::parse(&frame[..after_header]).unwrap_err().needed();
        assert!(
            needed >= elements * MIN_ELEMENT_BYTES,
            "bound {needed} ignores the {elements} outstanding elements"
        );
        assert!(needed <= frame.len(), "bound {needed} over-estimates");
    }

    /// Simulates the read loop: feed the frame one segment at a time, attempting
    /// a parse only once the buffer has reached the last reported bound. The
    /// frame must still parse, and the attempts must be well below the segment
    /// count.
    ///
    /// The reduction is a few-fold, not orders of magnitude, and that ceiling is
    /// inherent: the bound is `consumed + MIN_ELEMENT_BYTES × outstanding`, so
    /// as a frame nears its end the outstanding term shrinks while the consumed
    /// term grows byte-for-byte with the buffer, and the bound stops running
    /// ahead. Cutting the *cost* of the attempts that remain is `frame_len`'s
    /// job — see `frame_len_and_parse_agree_on_every_prefix` and the allocation
    /// bounds in `tests/resp_prealloc.rs`.
    #[test]
    fn gating_on_the_bound_cuts_parse_attempts_without_losing_the_frame() {
        const SEGMENT: usize = 1400; // a plausible TCP payload
        let frame = multibulk(4_000, 100);
        let segments = frame.len().div_ceil(SEGMENT);

        let mut buf: Vec<u8> = Vec::new();
        let mut need = 0usize;
        let mut attempts = 0usize;
        let mut parsed = None;

        for chunk in frame.chunks(SEGMENT) {
            buf.extend_from_slice(chunk);
            if buf.len() < need {
                continue;
            }
            attempts += 1;
            match Value::parse(&buf) {
                Ok((v, n)) => {
                    assert_eq!(n, frame.len());
                    parsed = Some(v);
                    break;
                }
                Err(e) => {
                    assert!(e.is_incomplete(), "unexpected {e:?}");
                    need = e.needed();
                }
            }
        }

        assert!(parsed.is_some(), "frame never completed under the gate");
        assert!(
            attempts * 2 < segments,
            "{attempts} parse attempts over {segments} segments — the bound is not \
             saving meaningful work"
        );
    }

    /// Ungated, the same frame is parsed once per segment. This is the
    /// behaviour being replaced; it is asserted so the comparison above is not
    /// measuring a frame that happened to arrive in one piece.
    #[test]
    fn without_the_gate_every_segment_costs_a_parse() {
        const SEGMENT: usize = 1400;
        let frame = multibulk(4_000, 100);
        let segments = frame.len().div_ceil(SEGMENT);
        assert!(segments > 100, "test frame should span many segments");

        let mut buf: Vec<u8> = Vec::new();
        let mut attempts = 0usize;
        for chunk in frame.chunks(SEGMENT) {
            buf.extend_from_slice(chunk);
            attempts += 1;
            if Value::parse(&buf).is_ok() {
                break;
            }
        }
        assert_eq!(attempts, segments);
    }
}
