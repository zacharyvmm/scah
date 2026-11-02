//! HTML character-reference decoding, for text and attribute values.
//!
//! Text decodes references as the HTML tokenizer's data state does.
//! Attribute values follow the same rules except one: a named reference
//! written without its `;` stays as written when an `=` or an ASCII letter
//! or digit follows it, so `?a=1&copy=2` keeps its `&copy`.
//!
//! Attribute values are also preprocessed as a browser's tokenizer sees
//! them: CRLF and a lone CR written in the HTML become LF, and NUL becomes
//! U+FFFD. A CR that a reference such as `&#13;` produces stays CR.

#[path = "entities_table.rs"]
mod entities_table;

use std::borrow::Cow;

use entities_table::{
    ENTITY_NAME_ENDS, ENTITY_NAMES, ENTITY_VALUE_ENDS, ENTITY_VALUES, MAX_NAME_LEN,
    NAMED_ENTITY_COUNT,
};

// Derive a small search index from the generated, sorted names. Searching
// only names with the same initial byte avoids unrelated table comparisons.
const SEARCH_INDEX: ([u16; 129], usize) = build_search_index();

const fn build_search_index() -> ([u16; 129], usize) {
    let mut starts = [0; 129];
    let mut index = 0;
    let mut initial = 0;
    let mut longest_legacy_name = 0;
    while initial < starts.len() {
        while index < NAMED_ENTITY_COUNT {
            let start = if index == 0 {
                0
            } else {
                ENTITY_NAME_ENDS[index - 1] as usize
            };
            if ENTITY_NAMES[start] as usize >= initial {
                break;
            }
            let end = ENTITY_NAME_ENDS[index] as usize;
            if ENTITY_NAMES[end - 1] != b';' && end - start > longest_legacy_name {
                longest_legacy_name = end - start;
            }
            index += 1;
        }
        starts[initial] = index as u16;
        initial += 1;
    }
    (starts, longest_legacy_name)
}

#[inline]
fn entity_name(index: usize) -> &'static [u8] {
    let start = if index == 0 {
        0
    } else {
        ENTITY_NAME_ENDS[index - 1] as usize
    };
    let end = ENTITY_NAME_ENDS[index] as usize;
    &ENTITY_NAMES[start..end]
}

#[inline]
fn entity_value(index: usize) -> &'static [u8] {
    let start = if index == 0 {
        0
    } else {
        ENTITY_VALUE_ENDS[index - 1] as usize
    };
    let end = ENTITY_VALUE_ENDS[index] as usize;
    &ENTITY_VALUES[start..end]
}

/// Return whether `source` might need character-reference decoding.
#[inline]
pub fn contains_ampersand(source: &str) -> bool {
    source.as_bytes().contains(&b'&')
}

/// Return whether the attribute value `value`, as written, might differ
/// once decoded by [`decode_attribute_value`]: whether it has `&`, CR, or
/// NUL. Inline, eight bytes at a time, with no call into `memchr`, since
/// attribute values are mostly short.
#[inline(always)]
pub fn value_needs_decoding(value: &str) -> bool {
    const ONES: u64 = u64::from_ne_bytes([0x01; 8]);
    const HIGHS: u64 = u64::from_ne_bytes([0x80; 8]);
    const AMPERSANDS: u64 = u64::from_ne_bytes([b'&'; 8]);
    const BELOW_SO: u64 = u64::from_ne_bytes([0x0E; 8]);
    // `&` is tested exactly. CR and NUL are tested with a cheaper test that
    // also takes in the other bytes below U+000E, such as tab, LF, and FF:
    // values with one of those are rare, and get the exact test.
    let bytes = value.as_bytes();
    let Some(&last) = bytes.last_chunk::<8>() else {
        for &byte in bytes {
            if byte < 0x0E || byte == b'&' {
                return byte == b'&' || bytes_need_decoding(bytes);
            }
        }
        return false;
    };
    // The high bit of each `&` byte, or of each byte below U+000E.
    let ampersands = |word: u64| {
        let ampersands = word ^ AMPERSANDS;
        ampersands.wrapping_sub(ONES) & !ampersands & HIGHS
    };
    let found = |word: u64| ampersands(word) | (word.wrapping_sub(BELOW_SO) & !word & HIGHS);
    let (chunks, _) = bytes.as_chunks::<8>();
    for &chunk in chunks {
        let word = u64::from_ne_bytes(chunk);
        if found(word) != 0 {
            return ampersands(word) != 0 || bytes_need_decoding(bytes);
        }
    }
    // The last eight bytes overlap the last whole chunk, rather than
    // leaving a tail to test a byte at a time.
    let word = u64::from_ne_bytes(last);
    found(word) != 0 && (ampersands(word) != 0 || bytes_need_decoding(bytes))
}

/// The exact test of [`value_needs_decoding`].
#[cold]
#[inline(never)]
fn bytes_need_decoding(bytes: &[u8]) -> bool {
    bytes.iter().any(|&byte| matches!(byte, b'&' | b'\r' | 0))
}

/// Decode HTML character references in `source` into `out`.
///
/// Decoded UTF-8 bytes are appended to `out`. Callers should use
/// [`contains_ampersand`] to fast-path fragments that cannot contain a
/// character reference.
#[cold]
#[inline(never)]
pub fn decode_character_references(source: &str, out: &mut Vec<u8>) {
    decode_character_references_impl::<false, false>(source, out);
}

/// Decode character references after applying HTML source-newline normalization.
///
/// Only CR and CRLF bytes copied from `source` become LF. A CR produced by a
/// numeric character reference remains CR because character references are
/// resolved after source preprocessing.
#[cold]
#[inline(never)]
pub fn decode_character_references_normalizing_source_newlines(source: &str, out: &mut Vec<u8>) {
    decode_character_references_impl::<true, false>(source, out);
}

/// Decode an attribute value into `out`, as a browser's tokenizer does:
/// character references by the attribute-value rules, CR and CRLF to LF,
/// and NUL to U+FFFD (see the [module](self) docs).
///
/// Appends whole UTF-8 sequences only, so `out` stays valid UTF-8 if it was.
#[cold]
#[inline(never)]
pub fn decode_attribute_value_into(source: &str, out: &mut Vec<u8>) {
    decode_character_references_impl::<true, true>(source, out);
}

/// An attribute value decoded as [`decode_attribute_value_into`] does:
/// `source` itself when it has no `&`, CR, or NUL, so most values are never
/// copied.
#[inline]
pub fn decode_attribute_value(source: &str) -> Cow<'_, str> {
    if !value_needs_decoding(source) {
        return Cow::Borrowed(source);
    }
    let mut out = Vec::with_capacity(source.len());
    decode_attribute_value_into(source, &mut out);
    debug_assert!(std::str::from_utf8(&out).is_ok());
    // SAFETY: the decoder copies `source` only between ASCII `&`, CR, LF and
    // NUL bytes, so at character boundaries, and otherwise appends whole
    // UTF-8 sequences.
    Cow::Owned(unsafe { String::from_utf8_unchecked(out) })
}

fn decode_character_references_impl<
    const NORMALIZE_SOURCE_NEWLINES: bool,
    const ATTRIBUTE: bool,
>(
    source: &str,
    out: &mut Vec<u8>,
) {
    // Source preprocessing: with `NORMALIZE_SOURCE_NEWLINES`, CR and CRLF
    // become LF, and in an `ATTRIBUTE` value NUL becomes U+FFFD, as the
    // tokenizer would see them. Only bytes copied from `source` change, not
    // those a reference produces.
    let special = |byte: u8| {
        byte == b'&' || (NORMALIZE_SOURCE_NEWLINES && byte == b'\r') || (ATTRIBUTE && byte == 0)
    };
    let bytes = source.as_bytes();
    let mut copied_through = 0;

    while let Some(relative) = bytes[copied_through..]
        .iter()
        .position(|&byte| special(byte))
    {
        let at = copied_through + relative;
        out.extend_from_slice(&bytes[copied_through..at]);
        copied_through = at + 1;
        match bytes[at] {
            b'\r' if NORMALIZE_SOURCE_NEWLINES => {
                out.push(b'\n');
                if bytes.get(copied_through) == Some(&b'\n') {
                    copied_through += 1;
                }
            }
            0 if ATTRIBUTE => out.extend_from_slice("\u{FFFD}".as_bytes()),
            _ => match decode_after_ampersand::<ATTRIBUTE>(&bytes[copied_through..], out) {
                Some(consumed) => copied_through += consumed,
                None => out.push(b'&'),
            },
        }
    }

    out.extend_from_slice(&bytes[copied_through..]);
}

/// Decode the bytes following an ampersand and return their consumed length.
fn decode_after_ampersand<const ATTRIBUTE: bool>(
    source: &[u8],
    out: &mut Vec<u8>,
) -> Option<usize> {
    if source.first() == Some(&b'#') {
        decode_numeric(source, out)
    } else {
        decode_named::<ATTRIBUTE>(source, out)
    }
}

fn decode_numeric(source: &[u8], out: &mut Vec<u8>) -> Option<usize> {
    let mut cursor = 1;
    let radix = if matches!(source.get(cursor), Some(b'x' | b'X')) {
        cursor += 1;
        16
    } else {
        10
    };
    let digits_start = cursor;
    let mut value = 0_u64;

    while let Some(&byte) = source.get(cursor) {
        let Some(digit) = digit_value(byte, radix) else {
            break;
        };
        value = value
            .saturating_mul(u64::from(radix))
            .saturating_add(u64::from(digit));
        cursor += 1;
    }

    if cursor == digits_start {
        return None;
    }
    if source.get(cursor) == Some(&b';') {
        cursor += 1;
    }

    append_code_point(normalize_numeric_reference(value), out);
    Some(cursor)
}

#[inline]
fn digit_value(byte: u8, radix: u32) -> Option<u32> {
    match byte {
        b'0'..=b'9' => Some(u32::from(byte - b'0')),
        b'a'..=b'f' if radix == 16 => Some(u32::from(byte - b'a') + 10),
        b'A'..=b'F' if radix == 16 => Some(u32::from(byte - b'A') + 10),
        _ => None,
    }
}

fn decode_named<const ATTRIBUTE: bool>(source: &[u8], out: &mut Vec<u8>) -> Option<usize> {
    // Escaped HTML most often uses these five complete spellings. Resolve
    // them before the full-table search. Requiring the semicolon leaves
    // longest-prefix and legacy semicolon-less matching to the general path.
    let common: Option<(&[u8], usize)> = match source {
        [b'a', b'm', b'p', b';', ..] => Some((b"&", 4)),
        [b'l', b't', b';', ..] => Some((b"<", 3)),
        [b'g', b't', b';', ..] => Some((b">", 3)),
        [b'q', b'u', b'o', b't', b';', ..] => Some((b"\"", 5)),
        [b'n', b'b', b's', b'p', b';', ..] => Some(("\u{00A0}".as_bytes(), 5)),
        _ => None,
    };
    if let Some((value, consumed)) = common {
        out.extend_from_slice(value);
        return Some(consumed);
    }

    let mut name_end = 0;
    while name_end < source.len()
        && name_end < MAX_NAME_LEN
        && source[name_end].is_ascii_alphanumeric()
    {
        name_end += 1;
    }

    if name_end == 0 {
        return None;
    }

    let candidate_end = if name_end < MAX_NAME_LEN && source.get(name_end) == Some(&b';') {
        name_end + 1
    } else {
        name_end
    };

    if let Ok(index) = binary_search_entity_name(&source[..candidate_end]) {
        return emit_named::<ATTRIBUTE>(source, index, candidate_end, out);
    }

    // A shorter match cannot include the terminal semicolon. Only legacy
    // semicolon-less spellings can succeed, and none exceed this length.
    let mut candidate_end = (candidate_end - 1).min(SEARCH_INDEX.1);
    while candidate_end > 0 {
        let candidate = &source[..candidate_end];
        if let Ok(index) = binary_search_entity_name(candidate) {
            return emit_named::<ATTRIBUTE>(source, index, candidate_end, out);
        }
        candidate_end -= 1;
    }

    None
}

/// Append the value of the entity at `index`, the longest name that starts
/// `source` and is `consumed` bytes long, and return `consumed`.
///
/// In an attribute value, a name matched without its `;` and followed by
/// `=` or an ASCII alphanumeric is not a reference, for historical reasons
/// (HTML's named character reference state): `None`, appending nothing.
#[inline]
fn emit_named<const ATTRIBUTE: bool>(
    source: &[u8],
    index: usize,
    consumed: usize,
    out: &mut Vec<u8>,
) -> Option<usize> {
    if ATTRIBUTE
        && source[consumed - 1] != b';'
        && source
            .get(consumed)
            .is_some_and(|&next| next == b'=' || next.is_ascii_alphanumeric())
    {
        return None;
    }
    out.extend_from_slice(entity_value(index));
    Some(consumed)
}

#[inline]
fn binary_search_entity_name(candidate: &[u8]) -> Result<usize, usize> {
    let initial = candidate[0] as usize;
    let mut left = SEARCH_INDEX.0[initial] as usize;
    let mut right = SEARCH_INDEX.0[initial + 1] as usize;
    while left < right {
        let mid = left + (right - left) / 2;
        match entity_name(mid).cmp(candidate) {
            std::cmp::Ordering::Less => left = mid + 1,
            std::cmp::Ordering::Greater => right = mid,
            std::cmp::Ordering::Equal => return Ok(mid),
        }
    }
    Err(left)
}

fn normalize_numeric_reference(value: u64) -> char {
    let value = match value {
        0 | 0xD800..=0xDFFF | 0x110000.. => return '\u{FFFD}',
        0x80 => 0x20AC,
        0x82 => 0x201A,
        0x83 => 0x0192,
        0x84 => 0x201E,
        0x85 => 0x2026,
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02C6,
        0x89 => 0x2030,
        0x8A => 0x0160,
        0x8B => 0x2039,
        0x8C => 0x0152,
        0x8E => 0x017D,
        0x91 => 0x2018,
        0x92 => 0x2019,
        0x93 => 0x201C,
        0x94 => 0x201D,
        0x95 => 0x2022,
        0x96 => 0x2013,
        0x97 => 0x2014,
        0x98 => 0x02DC,
        0x99 => 0x2122,
        0x9A => 0x0161,
        0x9B => 0x203A,
        0x9C => 0x0153,
        0x9E => 0x017E,
        0x9F => 0x0178,
        value => value,
    };

    // The invalid scalar values returned above have already been handled.
    char::from_u32(value as u32).unwrap_or('\u{FFFD}')
}

#[inline]
fn append_code_point(character: char, out: &mut Vec<u8>) {
    let mut encoded = [0; 4];
    out.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::{
        contains_ampersand, decode_attribute_value, decode_character_references,
        decode_character_references_normalizing_source_newlines, value_needs_decoding,
    };
    use std::borrow::Cow;

    fn decode(source: &str) -> String {
        let mut output = Vec::new();
        decode_character_references(source, &mut output);
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn detects_ampersands() {
        assert!(!contains_ampersand("plain text"));
        assert!(contains_ampersand("one &amp; two"));
    }

    #[test]
    fn value_scan_finds_ampersands_crs_and_nuls_at_every_offset() {
        assert!(!value_needs_decoding(""));
        for len in 1..40 {
            // LF, tab, and bytes one off `&`, CR, and NUL need no decoding.
            let plain = "a%'é\n\t\x0c\x01"
                .chars()
                .cycle()
                .take(len)
                .collect::<String>();
            assert!(!value_needs_decoding(&plain), "{plain:?}");
            for at in 0..plain.len() {
                if !plain.is_char_boundary(at) {
                    continue;
                }
                for special in ['&', '\r', '\0'] {
                    let mut value = plain.clone();
                    value.insert(at, special);
                    assert!(value_needs_decoding(&value), "{value:?}");
                }
            }
        }
    }

    #[test]
    fn decodes_named_references_with_semicolons() {
        assert_eq!(
            decode("&amp;&nbsp;&lt;&gt;&quot;&copy;&NotEqualTilde;"),
            "&\u{00A0}<>\"©\u{2242}\u{0338}"
        );
    }

    #[test]
    fn decodes_legacy_named_references_without_semicolons_in_data_state() {
        assert_eq!(decode("&amp"), "&");
        assert_eq!(decode("&copy test"), "© test");
        assert_eq!(decode("&ampersand"), "&ersand");
    }

    #[test]
    fn decodes_decimal_and_hex_numeric_references() {
        assert_eq!(decode("&#65; &#65 &#x41; &#X41"), "A A A A");
    }

    #[test]
    fn source_newline_normalization_preserves_reference_cr() {
        let mut output = Vec::new();
        decode_character_references_normalizing_source_newlines(
            "\r\n&amp;\r&#13;&#10;",
            &mut output,
        );
        assert_eq!(String::from_utf8(output).unwrap(), "\n&\n\r\n");
    }

    #[test]
    fn replaces_invalid_numeric_scalar_values() {
        assert_eq!(
            decode("&#0; &#xD800; &#x110000;"),
            "\u{FFFD} \u{FFFD} \u{FFFD}"
        );
    }

    #[test]
    fn remaps_windows_1252_numeric_references() {
        let remapped = [
            (0x80, '\u{20AC}'),
            (0x82, '\u{201A}'),
            (0x83, '\u{0192}'),
            (0x84, '\u{201E}'),
            (0x85, '\u{2026}'),
            (0x86, '\u{2020}'),
            (0x87, '\u{2021}'),
            (0x88, '\u{02C6}'),
            (0x89, '\u{2030}'),
            (0x8A, '\u{0160}'),
            (0x8B, '\u{2039}'),
            (0x8C, '\u{0152}'),
            (0x8E, '\u{017D}'),
            (0x91, '\u{2018}'),
            (0x92, '\u{2019}'),
            (0x93, '\u{201C}'),
            (0x94, '\u{201D}'),
            (0x95, '\u{2022}'),
            (0x96, '\u{2013}'),
            (0x97, '\u{2014}'),
            (0x98, '\u{02DC}'),
            (0x99, '\u{2122}'),
            (0x9A, '\u{0161}'),
            (0x9B, '\u{203A}'),
            (0x9C, '\u{0153}'),
            (0x9E, '\u{017E}'),
            (0x9F, '\u{0178}'),
        ];
        for (source, expected) in remapped {
            assert_eq!(decode(&format!("&#{source};")), expected.to_string());
        }
        for unchanged in [0x81_u32, 0x8D, 0x8F, 0x90, 0x9D] {
            assert_eq!(
                decode(&format!("&#{unchanged};")),
                char::from_u32(unchanged).unwrap().to_string()
            );
        }
    }

    #[test]
    fn preserves_malformed_references_without_skipping_input() {
        assert_eq!(
            decode("&unknown; & &#; &#x; &#X; end"),
            "&unknown; & &#; &#x; &#X; end"
        );
    }

    #[test]
    fn every_named_reference_matches_the_generated_value() {
        for index in 0..super::NAMED_ENTITY_COUNT {
            let name = std::str::from_utf8(super::entity_name(index)).unwrap();
            let value = std::str::from_utf8(super::entity_value(index)).unwrap();
            assert_eq!(decode(&format!("&{name}")), value, "{name}");
            assert_eq!(
                decode(&format!("&{name}!é")),
                format!("{value}!é"),
                "{name}"
            );
        }
        assert_eq!(
            decode("&amp;&amper;&AMP;&Amp;&notin;&notit;"),
            "&&er;&&Amp;∉¬it;"
        );
    }

    #[test]
    fn named_lookup_preserves_longest_prefix_for_extended_and_truncated_names() {
        for index in 0..super::NAMED_ENTITY_COUNT {
            let name = std::str::from_utf8(super::entity_name(index)).unwrap();
            for source in [
                format!("{name}letters;"),
                format!("{}invalid;", name.trim_end_matches(';')),
                name[..name.len() - 1].to_owned(),
            ] {
                let expected = (0..super::NAMED_ENTITY_COUNT)
                    .filter(|&candidate| {
                        source.as_bytes().starts_with(super::entity_name(candidate))
                    })
                    .max_by_key(|&candidate| super::entity_name(candidate).len());
                let mut output = Vec::new();
                let consumed = super::decode_named::<false>(source.as_bytes(), &mut output);
                assert_eq!(
                    consumed,
                    expected.map(|candidate| super::entity_name(candidate).len()),
                    "{source}"
                );
                assert_eq!(
                    output.as_slice(),
                    expected.map(super::entity_value).unwrap_or_default(),
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn attribute_values_without_ampersands_crs_or_nuls_are_borrowed() {
        let value = "/search?q=a;b\n\tc";
        let Cow::Borrowed(decoded) = decode_attribute_value(value) else {
            panic!("copied a value without references");
        };
        assert_eq!(decoded.as_ptr(), value.as_ptr());
    }

    #[test]
    fn attribute_values_decode_references_with_semicolons() {
        for (source, expected) in [
            ("&amp;", "&"),
            ("a&#x62;", "ab"),
            ("a=1&amp;b=2", "a=1&b=2"),
            ("&copy;=2", "©=2"),
            ("&notin;", "∉"),
            ("&NotEqualTilde;x", "\u{2242}\u{0338}x"),
        ] {
            assert_eq!(decode_attribute_value(source), expected, "{source:?}");
        }
    }

    #[test]
    fn attribute_values_keep_legacy_references_before_equals_or_alphanumerics() {
        for (source, expected) in [
            // Followed by `=` or an alphanumeric: not a reference.
            ("?a=1&copy=2", "?a=1&copy=2"),
            ("&ampx", "&ampx"),
            ("&amp1", "&amp1"),
            ("&notit;", "&notit;"),
            // Followed by anything else, or nothing: decoded as in text.
            ("&copy 2", "© 2"),
            ("&copy", "©"),
            ("&amp&lt", "&<"),
            ("&copy-2", "©-2"),
        ] {
            assert_eq!(decode_attribute_value(source), expected, "{source:?}");
        }
        // Text decodes the same legacy references.
        assert_eq!(decode("?a=1&copy=2&notit;"), "?a=1©=2¬it;");
    }

    #[test]
    fn attribute_values_decode_numeric_references_as_text_does() {
        for (source, expected) in [
            ("&#x62;", "b"),
            ("&#98", "b"),
            ("&#x62x", "bx"),
            ("&#0;", "\u{FFFD}"),
            ("&#x80;", "€"),
            ("&#xD800;", "\u{FFFD}"),
            ("&#;", "&#;"),
            ("& &x; &#x;", "& &x; &#x;"),
        ] {
            assert_eq!(decode_attribute_value(source), expected, "{source:?}");
        }
    }

    #[test]
    fn attribute_values_normalize_source_newlines_and_nuls() {
        for (source, expected) in [
            ("a\r\nb", "a\nb"),
            ("c\rd", "c\nd"),
            ("\r\r\n\n\r", "\n\n\n\n"),
            ("x\0y\0", "x\u{FFFD}y\u{FFFD}"),
            ("é\0\r\né", "é\u{FFFD}\né"),
            // References compose: only CR and NUL written in the HTML change.
            ("a&amp;\r\nb", "a&\nb"),
            ("&#13;&#10;\r\n", "\r\n\n"),
            ("&#x0D;\0&#0;", "\r\u{FFFD}\u{FFFD}"),
            ("&\r&\0", "&\n&\u{FFFD}"),
            ("&copy\r\n", "©\n"),
            ("&am\0p;", "&am\u{FFFD}p;"),
        ] {
            assert_eq!(decode_attribute_value(source), expected, "{source:?}");
        }
        // Text keeps its NUL; only preformatted text normalizes newlines.
        assert_eq!(decode("a\0\r\n&amp;"), "a\0\r\n&");
        let mut output = Vec::new();
        decode_character_references_normalizing_source_newlines("a\0\r\n&amp;", &mut output);
        assert_eq!(output, b"a\0\n&");
    }

    #[test]
    fn appends_to_existing_output() {
        let mut output = b"prefix: ".to_vec();
        decode_character_references("&lt;x&gt;", &mut output);
        assert_eq!(output, b"prefix: <x>");
    }
}
