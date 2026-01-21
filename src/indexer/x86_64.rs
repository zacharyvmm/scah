use std::arch::x86_64::{__m512i, _mm512_cmpeq_epi8_mask, _mm512_set_epi64, _mm512_set1_epi8};

const LOW_BIT_MASK: u64 = 0x0101010101010101;
const HIGH_BIT_MASK: u64 = 0x8080808080808080;
const ODD_BITS: u64 = 0xAAAAAAAAAAAAAAAA;

#[inline(always)]
unsafe fn compare(haystack: __m512i, needle: u8) -> u64 {
    let pattern: __m512i = _mm512_set1_epi8(needle as i8);

    // returns a u64 (1bit representing a match for every byte)
    _mm512_cmpeq_epi8_mask(haystack, pattern)
}

fn escaped(haystack: __m512i) -> u64 {
    let backslashes = unsafe { compare(haystack, b'\\') };
    let maybe_escaped = backslashes << 1;

    let maybe_escaped_and_odd_bits = maybe_escaped | ODD_BITS;
    let even_series_codes_and_odd_bits = maybe_escaped_and_odd_bits.wrapping_sub(backslashes);
    let escape_and_terminal_code = even_series_codes_and_odd_bits ^ ODD_BITS;

    let escaped = escape_and_terminal_code ^ backslashes;
    escaped
}

unsafe fn structural_mask(haystack: __m512i) -> u64 {
    let matches = compare(haystack, b'<')
        | compare(haystack, b'>')
        | compare(haystack, b' ')
        | compare(haystack, b'"')
        | compare(haystack, b'\'')
        | compare(haystack, b'=')
        | compare(haystack, b'/')
        | compare(haystack, b'!');

    matches & !escaped(haystack)
}

#[inline(always)]
unsafe fn get_512_word(ptr: *const u8, offset: usize) -> __m512i {
    let e0 = (ptr.add(offset) as *const i64).read_unaligned();
    let e1 = (ptr.add(offset + 8) as *const i64).read_unaligned();
    let e2 = (ptr.add(offset + 2 * 8) as *const i64).read_unaligned();
    let e3 = (ptr.add(offset + 3 * 8) as *const i64).read_unaligned();
    let e4 = (ptr.add(offset + 4 * 8) as *const i64).read_unaligned();
    let e5 = (ptr.add(offset + 5 * 8) as *const i64).read_unaligned();
    let e6 = (ptr.add(offset + 6 * 8) as *const i64).read_unaligned();
    let e7 = (ptr.add(offset + 7 * 8) as *const i64).read_unaligned();

    _mm512_set_epi64(e7, e6, e5, e4, e3, e2, e1, e0)
}

const RATIO_DENOMINATOR: usize = 8;

fn _parse(buffer: &[u8]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(buffer.len() / RATIO_DENOMINATOR);
    let ptr = buffer.as_ptr();

    // Iterate only up to original length
    let len = buffer.len() - 8;

    let mut i = 0;
    const STEP: usize = 64; // 512/8 = 64
    while i < len {
        let word = unsafe { get_512_word(ptr, i) };

        let mut mask = unsafe { structural_mask(word) };

        while mask != 0 {
            let byte_offset = mask.trailing_zeros();
            let index = byte_offset + i as u32;
            out.push(index);
            mask &= mask - 1;
        }

        i += STEP;
    }

    out
}

fn buffer(input: &str) -> Vec<u8> {
    let mut buffer = input.as_bytes().to_vec();

    // Add 64 (512/8 = 64) null bytes to the end
    buffer.extend_from_slice(&[0u8; 64]);

    buffer
}

pub fn parse(input: &str) -> Vec<u32> {
    let buffer = buffer(input);
    let indices = _parse(&buffer);
    indices
}

#[cfg(test)]
mod tests {
    use super::super::swar;
    use super::*;
    #[test]
    fn test_comparison_x86_64() {
        let string = r#"<    div   >HEllo World <a href="link" class="\"my class\""> HERe  \</ a href="Fake link<span> Hello </span>"\>\<\a\></a><   /  div >"#;
        let indices = parse(string);

        assert_eq!(indices, swar::parse(string));
    }
}
