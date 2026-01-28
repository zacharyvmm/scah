use super::SIMD;
const HIGH_BIT_MASK: u64 = 0x8080808080808080;
const LOW_BIT_MASK: u64 = 0x0101010101010101;

#[inline(always)]
fn compare(haystack: u64, needle: u8) -> u64 {
    let pattern = (needle as u64) * LOW_BIT_MASK;
    let comparison = haystack ^ pattern;

    comparison.wrapping_sub(LOW_BIT_MASK) & !comparison
}

#[inline(always)]
fn compare_range(haystack: u64, range: std::ops::Range<u8>) -> u64 {
    const LOW_7_BITS: u64 = 0x7F7F7F7F7F7F7F7F;
    let low_bounds = LOW_BIT_MASK * range.start as u64;
    let high_bounds = LOW_BIT_MASK * (range.end as u64);

    let res_low = (haystack | HIGH_BIT_MASK).wrapping_sub(low_bounds);
    let res_high = (high_bounds | HIGH_BIT_MASK).wrapping_sub(haystack & LOW_7_BITS);

    res_low & res_high
}
pub struct SWAR;
impl SIMD for SWAR {
    type RegisterSize = u64;
    const BYTES: usize = u64::BITS as usize / 8;

    // fn structural_mask(haystack: Self::RegisterSize) -> u64 {
    //     let matches = compare(haystack, b'<')
    //         | compare(haystack, b'>')
    //         | compare(haystack, b' ')
    //         | compare(haystack, b'"')
    //         | compare(haystack, b'\'')
    //         | compare(haystack, b'=')
    //         | compare(haystack, b'/')
    //         | compare(haystack, b'!');

    //     matches
    // }

    fn structural_mask(haystack: Self::RegisterSize) -> u64 {
        let matches = compare_range(haystack, b' '..b'"') // ` `, `!`, `"`
            | compare_range(haystack, b'<'..b'>') // `<`, `=`, `>`
            | compare(haystack, b'\'') | compare(haystack, b'/');

        matches
    }

    #[inline(always)]
    fn next_escape_and_terminal_code(backslashes: u64) -> u64 {
        const BACKSLASH_OFFSET: u64 = 8;
        const ODD_MASK: u64 = 0x0080008000800080;

        let maybe_escaped = backslashes << BACKSLASH_OFFSET;
        let maybe_escaped_and_odd_bits = maybe_escaped | ODD_MASK;
        let even_series_codes_and_odd_bits = maybe_escaped_and_odd_bits.wrapping_sub(backslashes);
        even_series_codes_and_odd_bits ^ ODD_MASK
    }

    #[inline(always)]
    fn escaped(haystack: Self::RegisterSize, next_is_escaped: u64) -> (u64, u64) {
        let haystack = haystack & !next_is_escaped;

        let backslashes = compare(haystack, b'\\') & HIGH_BIT_MASK;
        let escape_and_terminal_code = Self::next_escape_and_terminal_code(backslashes);

        let escaped = escape_and_terminal_code ^ (backslashes | next_is_escaped);
        let escape = escape_and_terminal_code & backslashes;
        (escaped, escape)
    }

    #[inline(always)]
    fn trailing_zeros(mask: u64) -> u32 {
        //mask.trailing_zeros() / 8
        mask.trailing_zeros() >> 3
    }

    #[inline(always)]
    fn filter(mask: u64) -> u64 {
        mask & HIGH_BIT_MASK
    }

    #[inline(always)]
    fn next_is_escaped(mask: u64) -> u64 {
        mask >> 56
    }

    #[inline(always)]
    fn get_word(ptr: *const u8, offset: usize) -> Self::RegisterSize {
        let as_64_block = unsafe { ptr.add(offset) } as *const u64;
        unsafe { as_64_block.read_unaligned() }
    }

    #[inline(always)]
    fn get_word_aligned(buffer: &[u64], offset: usize) -> Self::RegisterSize {
        buffer[offset]
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::scanner::Scanner;
    use super::*;

    #[test]
    fn test_bulk_swar_indexing() {
        let string = "<div class=\"hello-world\" id=hello-world>Hello World</div>";

        let buffer = SWAR::buffer(string);
        const RATIO_DENOMINATOR: usize = 8;
        let mut indices: Vec<u32> = Vec::with_capacity(string.len() / RATIO_DENOMINATOR);
        Scanner::new().scan::<SWAR>(
            &mut indices,
            0,
            buffer.as_slice(),
            buffer.len() - SWAR::BYTES,
        );

        let where_open: Vec<u32> = string
            .char_indices()
            .filter(|(_, c)| *c == '<')
            .map(|(i, _)| i as u32)
            .collect();
        let where_close: Vec<u32> = string
            .char_indices()
            .filter(|(_, c)| *c == '>')
            .map(|(i, _)| i as u32)
            .collect();
        let where_space: Vec<u32> = string
            .char_indices()
            .filter(|(_, c)| *c == ' ')
            .map(|(i, _)| i as u32)
            .collect();
        let where_dq: Vec<u32> = string
            .char_indices()
            .filter(|(_, c)| *c == '"')
            .map(|(i, _)| i as u32)
            .collect();
        let where_sq: Vec<u32> = string
            .char_indices()
            .filter(|(_, c)| *c == '\'')
            .map(|(i, _)| i as u32)
            .collect();
        let where_equal: Vec<u32> = string
            .char_indices()
            .filter(|(_, c)| *c == '=')
            .map(|(i, _)| i as u32)
            .collect();
        let where_slash: Vec<u32> = string
            .char_indices()
            .filter(|(_, c)| *c == '/')
            .map(|(i, _)| i as u32)
            .collect();

        let mut slow_indices: Vec<u32> = Vec::new();

        slow_indices.extend(where_open);
        slow_indices.extend(where_close);
        slow_indices.extend(where_space);
        slow_indices.extend(where_dq);
        slow_indices.extend(where_sq);
        slow_indices.extend(where_equal);
        slow_indices.extend(where_slash);

        slow_indices.sort();

        assert_eq!(indices, slow_indices);
    }

    #[test]
    fn test_html() {
        let string = "ello </s";
        let expected_indices = vec![4, 5, 6];
        // let string = "<<<< <<<";
        // let expected_indices = vec![0, 1, 2, 3, 4, 5, 6, 7];

        let buffer = SWAR::buffer(string);
        const RATIO_DENOMINATOR: usize = 8;
        let mut indices: Vec<u32> = Vec::with_capacity(string.len() / RATIO_DENOMINATOR);
        Scanner::new().scan::<SWAR>(
            &mut indices,
            0,
            buffer.as_slice(),
            buffer.len() - SWAR::BYTES,
        );

        assert_eq!(indices, expected_indices);
    }

    #[test]
    fn test_find_escaped_characters() {
        let string = b"\\ \\ \\ \\n";
        let num = u64::from_le_bytes(*string);
        let escaped = SWAR::escaped(num, 0).0 & HIGH_BIT_MASK;

        let expected = &[0, 0x80, 0, 0x80, 0, 0x80, 0, 0x80];
        let expected = u64::from_le_bytes(*expected);
        assert_eq!(
            escaped, expected,
            "ERR:\nescaped:\t{:064b}\nexpected:\t{:064b}",
            escaped, expected
        );
    }

    #[test]
    fn test_find_chained_escaped_characters() {
        let string = b"\\\\\\n  \\n";
        let num = u64::from_le_bytes(*string);
        println!("{:064b}", num);
        let escaped = SWAR::escaped(num, 0).0 & HIGH_BIT_MASK;

        let expected = &[0, 0x80, 0, 0x80, 0, 0, 0, 0x80];
        let expected = u64::from_le_bytes(*expected);

        assert_eq!(
            escaped, expected,
            "ERR:\nescaped:\t{:064b}\nexpected:\t{:064b}",
            escaped, expected
        );
    }
}
