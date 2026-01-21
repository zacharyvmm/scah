use std::arch::x86_64::{
    __m512i, _mm512_andnot_si512, _mm512_cmpeq_epi8_mask, _mm512_set_epi64, _mm512_set1_epi8,
};

#[inline(always)]
fn cast_to_i64(ptr: *const u8, offset: usize) -> i64 {
    unsafe { (ptr.add(offset) as *const i64).read_unaligned() }
}

use super::SIMD;
pub struct SIMD512;

impl SIMD for SIMD512 {
    type RegisterSize = __m512i;
    const BYTES: usize = 512 / 8;

    #[inline(always)]
    fn compare(haystack: __m512i, needle: u8) -> u64 {
        let pattern: __m512i = unsafe { _mm512_set1_epi8(needle as i8) };

        // returns a u64 (1bit representing a match for every byte)
        unsafe { _mm512_cmpeq_epi8_mask(haystack, pattern) }
    }

    // https://github.com/simdjson/simdjson/blob/master/src/generic/stage1/json_escape_scanner.h
    #[inline(always)]
    fn escaped(haystack: Self::RegisterSize, next_is_escaped: u64) -> u64 {
        const BACKSLASH_OFFSET: u64 = 1;
        const ODD_MASK: u64 = 0xAAAAAAAAAAAAAAAA;

        let mask = unsafe { _mm512_set_epi64(next_is_escaped as i64, 0, 0, 0, 0, 0, 0, 0) };

        let haystack = unsafe { _mm512_andnot_si512(mask, haystack) };
        let backslashes = Self::compare(haystack, b'\\');
        let maybe_escaped = backslashes << BACKSLASH_OFFSET;

        let maybe_escaped_and_odd_bits = maybe_escaped | ODD_MASK;
        let even_series_codes_and_odd_bits = maybe_escaped_and_odd_bits.wrapping_sub(backslashes);
        let escape_and_terminal_code = even_series_codes_and_odd_bits ^ ODD_MASK;

        let escaped = escape_and_terminal_code ^ backslashes;
        escaped
    }

    #[inline(always)]
    fn get_word(ptr: *const u8, offset: usize) -> Self::RegisterSize {
        let e0 = cast_to_i64(ptr, offset);
        let e1 = cast_to_i64(ptr, offset + 8);
        let e2 = cast_to_i64(ptr, offset + 2 * 8);
        let e3 = cast_to_i64(ptr, offset + 3 * 8);
        let e4 = cast_to_i64(ptr, offset + 4 * 8);
        let e5 = cast_to_i64(ptr, offset + 5 * 8);
        let e6 = cast_to_i64(ptr, offset + 6 * 8);
        let e7 = cast_to_i64(ptr, offset + 7 * 8);

        unsafe { _mm512_set_epi64(e7, e6, e5, e4, e3, e2, e1, e0) }
    }
}

#[cfg(test)]
mod tests {
    use super::super::swar::SWAR;
    use super::*;
}
