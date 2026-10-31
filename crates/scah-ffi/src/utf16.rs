//! UTF-8 HTML from UTF-16 strings, such as Java's and .NET's.
//!
//! Encoding a UTF-16 string as UTF-8 on the binding's side and then copying
//! and validating it here costs three passes over the document. A
//! [`Utf16Encoder`] makes one: it writes the UTF-8 straight into the buffer
//! the store will own, and needs no validation because it only writes whole
//! UTF-8 sequences.

use std::fmt;

/// What to do with a surrogate that is not half of a pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoneSurrogates {
    /// Replace it with U+FFFD, as the Encoding Standard does.
    Replace,
    /// Fail with a [`LoneSurrogateError`].
    Reject,
}

/// A lone surrogate in input that rejects them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoneSurrogateError {
    /// The surrogate.
    pub unit: u16,
    /// Its offset in UTF-16 units from the start of the input.
    pub offset: usize,
}

impl fmt::Display for LoneSurrogateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lone surrogate U+{:04X} at UTF-16 offset {}",
            self.unit, self.offset
        )
    }
}

impl std::error::Error for LoneSurrogateError {}

/// Encode `units` as UTF-8.
pub fn utf16_to_string(
    units: &[u16],
    lone_surrogates: LoneSurrogates,
) -> Result<String, LoneSurrogateError> {
    let mut encoder = Utf16Encoder::with_capacity(units.len(), lone_surrogates);
    encoder.push(units)?;
    encoder.finish()
}

/// Encodes UTF-16 that arrives in pieces as one UTF-8 string.
///
/// A surrogate pair may be split between two pieces.
#[derive(Debug)]
pub struct Utf16Encoder {
    /// Only ever extended by whole UTF-8 sequences, so always valid UTF-8.
    bytes: Vec<u8>,
    /// A high surrogate that ended the last piece, waiting for its pair.
    pending: Option<u16>,
    /// UTF-16 units pushed so far, to report the offsets of lone surrogates.
    units: usize,
    lone_surrogates: LoneSurrogates,
}

impl Utf16Encoder {
    /// An encoder with room for `capacity` bytes of UTF-8, which is exact for
    /// ASCII input of `capacity` units.
    pub fn with_capacity(capacity: usize, lone_surrogates: LoneSurrogates) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            pending: None,
            units: 0,
            lone_surrogates,
        }
    }

    /// The number of UTF-8 bytes encoded so far.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether nothing has been encoded yet.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty() && self.pending.is_none()
    }

    /// Encode the next piece of the input.
    ///
    /// On failure the encoder is left as it was before the call.
    pub fn push(&mut self, units: &[u16]) -> Result<(), LoneSurrogateError> {
        let (len, pending) = (self.bytes.len(), self.pending);
        let result = self.encode(units);
        match result {
            Ok(()) => self.units += units.len(),
            Err(_) => {
                self.bytes.truncate(len);
                self.pending = pending;
            }
        }
        result
    }

    /// The UTF-8 encoding of everything pushed.
    pub fn finish(mut self) -> Result<String, LoneSurrogateError> {
        if let Some(high) = self.pending.take() {
            self.lone(high, self.units - 1)?;
        }
        debug_assert!(std::str::from_utf8(&self.bytes).is_ok());
        // SAFETY: `bytes` only ever receives whole UTF-8 sequences: ASCII
        // bytes, and the encodings of scalar values and U+FFFD below.
        Ok(unsafe { String::from_utf8_unchecked(self.bytes) })
    }

    fn encode(&mut self, units: &[u16]) -> Result<(), LoneSurrogateError> {
        let mut i = 0;
        if let Some(high) = self.pending.take() {
            match units.first() {
                None => {
                    self.pending = Some(high);
                    return Ok(());
                }
                Some(&low) if is_low_surrogate(low) => {
                    self.push_scalar(combine(high, low));
                    i = 1;
                }
                Some(_) => self.lone(high, self.units - 1)?,
            }
        }
        while i < units.len() {
            i += push_ascii(&units[i..], &mut self.bytes);
            // Encode one block one unit at a time, then look for ASCII again.
            let end = (i + ASCII_BLOCK).min(units.len());
            while i < end {
                let unit = units[i];
                i += 1;
                if unit < 0x80 {
                    self.bytes.push(unit as u8);
                } else if !is_surrogate(unit) {
                    self.push_scalar(u32::from(unit));
                } else if is_low_surrogate(unit) {
                    self.lone(unit, self.units + i - 1)?;
                } else if i == units.len() {
                    // The pair may continue in the next piece.
                    self.pending = Some(unit);
                } else if is_low_surrogate(units[i]) {
                    self.push_scalar(combine(unit, units[i]));
                    i += 1;
                } else {
                    self.lone(unit, self.units + i - 1)?;
                }
            }
        }
        Ok(())
    }

    fn lone(&mut self, unit: u16, offset: usize) -> Result<(), LoneSurrogateError> {
        match self.lone_surrogates {
            LoneSurrogates::Replace => {
                self.push_scalar(0xFFFD);
                Ok(())
            }
            LoneSurrogates::Reject => Err(LoneSurrogateError { unit, offset }),
        }
    }

    /// Append a non-ASCII scalar value.
    fn push_scalar(&mut self, scalar: u32) {
        let continuation = |shift: u32| 0x80 | ((scalar >> shift) & 0x3F) as u8;
        if scalar < 0x800 {
            self.bytes
                .extend_from_slice(&[0xC0 | (scalar >> 6) as u8, continuation(0)]);
        } else if scalar < 0x10000 {
            self.bytes.extend_from_slice(&[
                0xE0 | (scalar >> 12) as u8,
                continuation(6),
                continuation(0),
            ]);
        } else {
            self.bytes.extend_from_slice(&[
                0xF0 | (scalar >> 18) as u8,
                continuation(12),
                continuation(6),
                continuation(0),
            ]);
        }
    }
}

fn is_surrogate(unit: u16) -> bool {
    (0xD800..0xE000).contains(&unit)
}

fn is_low_surrogate(unit: u16) -> bool {
    (0xDC00..0xE000).contains(&unit)
}

fn combine(high: u16, low: u16) -> u32 {
    0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(low) - 0xDC00)
}

/// Units the ASCII fast path checks and copies at a time.
const ASCII_BLOCK: usize = 32;

/// Append the ASCII blocks that start `units`, stopping at the first block
/// with a non-ASCII unit, and return how many units that was.
#[cfg(target_arch = "x86_64")]
fn push_ascii(units: &[u16], bytes: &mut Vec<u8>) -> usize {
    use std::arch::x86_64::{
        __m128i, _mm_and_si128, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_or_si128,
        _mm_packus_epi16, _mm_set1_epi16, _mm_setzero_si128, _mm_storeu_si128,
    };

    if units.len() < ASCII_BLOCK {
        return 0;
    }
    bytes.reserve(units.len());
    let start = bytes.len();
    let mut i = 0;
    // SAFETY: SSE2 is part of the x86_64 baseline. Each iteration reads the
    // 32 units at `i`, which `i + 32 <= units.len()` keeps in bounds, and
    // writes 32 bytes at `start + i`, within the `units.len()` bytes reserved
    // past `start`. Only ASCII is written, and `set_len` covers exactly the
    // bytes written.
    unsafe {
        let out = bytes.as_mut_ptr().add(start);
        let non_ascii = _mm_set1_epi16(0xFF80u16 as i16);
        while i + ASCII_BLOCK <= units.len() {
            let src = units.as_ptr().add(i).cast::<__m128i>();
            let a = _mm_loadu_si128(src);
            let b = _mm_loadu_si128(src.add(1));
            let c = _mm_loadu_si128(src.add(2));
            let d = _mm_loadu_si128(src.add(3));
            let any = _mm_or_si128(_mm_or_si128(a, b), _mm_or_si128(c, d));
            let high = _mm_and_si128(any, non_ascii);
            if _mm_movemask_epi8(_mm_cmpeq_epi8(high, _mm_setzero_si128())) != 0xFFFF {
                break;
            }
            let dst = out.add(i).cast::<__m128i>();
            _mm_storeu_si128(dst, _mm_packus_epi16(a, b));
            _mm_storeu_si128(dst.add(1), _mm_packus_epi16(c, d));
            i += ASCII_BLOCK;
        }
        bytes.set_len(start + i);
    }
    i
}

#[cfg(not(target_arch = "x86_64"))]
use push_ascii_portable as push_ascii;

/// [`push_ascii`] for any target.
#[cfg(any(not(target_arch = "x86_64"), test))]
fn push_ascii_portable(units: &[u16], bytes: &mut Vec<u8>) -> usize {
    let mut i = 0;
    // Fixed-size blocks let the compiler vectorize both loops.
    for block in units.as_chunks::<ASCII_BLOCK>().0 {
        if block.iter().fold(0, |any, &unit| any | unit) >= 0x80 {
            break;
        }
        bytes.extend_from_slice(&block.map(|unit| unit as u8));
        i += ASCII_BLOCK;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lossy(units: &[u16]) -> String {
        String::from_utf16_lossy(units)
    }

    /// Strings that cover every encoding length, both surrogate orders, and
    /// ASCII runs long enough for the fast path, around non-ASCII units.
    fn samples() -> Vec<Vec<u16>> {
        let ascii: Vec<u16> = "<div class=\"a\">plain ascii text</div>\n"
            .repeat(3)
            .encode_utf16()
            .collect();
        let mut samples = vec![
            vec![],
            ascii.clone(),
            "é€😀 Pöst <a>Публикация</a> 漢字".encode_utf16().collect(),
        ];
        let odd: [&[u16]; 8] = [
            &[0xD800],
            &[0xDC00],
            &[0xDC00, 0xD800],
            &[0xD83D, 0xDE00],
            &[0xD83D, 0x41],
            &[0x41, 0xDFFF, 0xD800, 0xD800, 0xDC00],
            &[0x7F, 0x80, 0x7FF, 0x800, 0xFFFF],
            &[0xD7FF, 0xE000, 0xFFFD],
        ];
        for units in odd {
            for at in [0, 1, 31, 32, 33, ascii.len()] {
                let mut sample = ascii.clone();
                sample.splice(at..at, units.iter().copied());
                samples.push(sample);
            }
        }
        samples
    }

    #[test]
    fn ascii_fast_paths_agree() {
        for units in samples() {
            for start in 0..units.len().min(40) {
                let (mut fast, mut portable) = (b"x".to_vec(), b"x".to_vec());
                let consumed = push_ascii(&units[start..], &mut fast);
                assert_eq!(
                    push_ascii_portable(&units[start..], &mut portable),
                    consumed
                );
                assert_eq!(fast, portable);
                assert_eq!(consumed % ASCII_BLOCK, 0);
                assert!(
                    units[start..start + consumed]
                        .iter()
                        .all(|&unit| unit < 0x80)
                );
                assert_eq!(
                    &fast[1..],
                    &lossy(&units[start..start + consumed]).into_bytes()[..]
                );
            }
        }
    }

    #[test]
    fn replaces_lone_surrogates_like_std() {
        for units in samples() {
            assert_eq!(
                utf16_to_string(&units, LoneSurrogates::Replace).unwrap(),
                lossy(&units),
                "{units:04X?}"
            );
        }
    }

    #[test]
    fn rejects_lone_surrogates_like_std() {
        for units in samples() {
            let expected = char::decode_utf16(units.iter().copied())
                .find_map(Result::err)
                .map(|error| error.unpaired_surrogate());
            match utf16_to_string(&units, LoneSurrogates::Reject) {
                Ok(text) => {
                    assert_eq!(expected, None, "{units:04X?}");
                    assert_eq!(text, lossy(&units));
                }
                Err(error) => {
                    assert_eq!(Some(error.unit), expected, "{units:04X?}");
                    assert_eq!(units[error.offset], error.unit);
                }
            }
        }
    }

    #[test]
    fn pieces_encode_like_the_whole() {
        for units in samples() {
            for split in 0..=units.len() {
                let mut encoder = Utf16Encoder::with_capacity(0, LoneSurrogates::Replace);
                encoder.push(&units[..split]).unwrap();
                encoder.push(&[]).unwrap();
                encoder.push(&units[split..]).unwrap();
                assert_eq!(
                    encoder.finish().unwrap(),
                    lossy(&units),
                    "{units:04X?} at {split}"
                );
            }
        }
    }

    #[test]
    fn a_pair_split_between_pieces_is_joined() {
        let mut encoder = Utf16Encoder::with_capacity(0, LoneSurrogates::Reject);
        encoder.push(&[0x41, 0xD83D]).unwrap();
        encoder.push(&[0xDE00]).unwrap();
        assert_eq!(encoder.finish().unwrap(), "A😀");
    }

    #[test]
    fn a_trailing_high_surrogate_is_lone() {
        let mut encoder = Utf16Encoder::with_capacity(0, LoneSurrogates::Reject);
        encoder.push(&[0x41, 0xD83D]).unwrap();
        let error = encoder.finish().unwrap_err();
        assert_eq!((error.unit, error.offset), (0xD83D, 1));

        assert_eq!(
            utf16_to_string(&[0x41, 0xD83D], LoneSurrogates::Replace).unwrap(),
            "A\u{FFFD}"
        );
    }

    #[test]
    fn offsets_count_from_the_first_piece() {
        let mut encoder = Utf16Encoder::with_capacity(0, LoneSurrogates::Reject);
        encoder.push(&[0x41; 40]).unwrap();
        let error = encoder.push(&[0x42, 0x43, 0xDC00]).unwrap_err();
        assert_eq!((error.unit, error.offset), (0xDC00, 42));
        assert_eq!(
            error.to_string(),
            "lone surrogate U+DC00 at UTF-16 offset 42"
        );

        let mut encoder = Utf16Encoder::with_capacity(0, LoneSurrogates::Reject);
        encoder.push(&[0x41, 0xD800]).unwrap();
        let error = encoder.push(&[0x42]).unwrap_err();
        assert_eq!((error.unit, error.offset), (0xD800, 1));
    }

    #[test]
    fn a_failed_push_changes_nothing() {
        let mut encoder = Utf16Encoder::with_capacity(0, LoneSurrogates::Reject);
        encoder.push(&[0x41, 0xD83D]).unwrap();
        let ascii = [0x61; 64];
        let mut bad = ascii.to_vec();
        bad.push(0xDC00);
        // The pending high surrogate pairs with nothing here, so this fails.
        assert!(encoder.push(&bad).is_err());
        assert_eq!(encoder.len(), 1);
        encoder.push(&[0xDE00]).unwrap();
        assert_eq!(encoder.finish().unwrap(), "A😀");
    }
}
