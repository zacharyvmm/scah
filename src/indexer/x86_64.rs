use std::arch::x86_64::{
    __m512i, _mm512_and_si512, _mm512_andnot_epi64, _mm512_lzcnt_epi64, _mm512_or_si512,
    _mm512_set_epi64, _mm512_set1_epi8, _mm512_set1_epi64, _mm512_srli_epi64, _mm512_storeu_epi64,
    _mm512_sub_epi64, _mm512_xor_si512,
};

const LOW_BIT_MASK: i64 = 0x0101010101010101;
//const HIGH_BIT_MASK: i64 = 0x8080808080808080;

#[inline(always)]
unsafe fn compare(haystack: __m512i, needle: u8) -> __m512i {
    let pattern: __m512i = _mm512_set1_epi8(needle as i8);
    let comparison = _mm512_xor_si512(haystack, pattern);

    //comparison.wrapping_sub(LOW_BIT_MASK) & !comparison
    let low_bit_mask = _mm512_set1_epi64(LOW_BIT_MASK);

    let substitution = _mm512_sub_epi64(comparison, low_bit_mask);

    _mm512_andnot_epi64(substitution, comparison)
}

#[inline(always)]
unsafe fn structural_mask(haystack: __m512i) -> __m512i {
    let open_cmp = compare(haystack, b'<');
    let close_cmp = compare(haystack, b'>');
    let space_cmp = compare(haystack, b' ');
    let dq_cmp = compare(haystack, b'"');
    let sq_cmp = compare(haystack, b'\'');
    let eq_cmp = compare(haystack, b'=');
    let fs_cmp = compare(haystack, b'/');
    let cm_cmp = compare(haystack, b'!');
    let matches = {
        let or = _mm512_or_si512(open_cmp, close_cmp);
        let or = _mm512_or_si512(or, space_cmp);
        let or = _mm512_or_si512(or, dq_cmp);
        let or = _mm512_or_si512(or, sq_cmp);
        let or = _mm512_or_si512(or, eq_cmp);
        let or = _mm512_or_si512(or, fs_cmp);
        let or = _mm512_or_si512(or, cm_cmp);
        or
    };

    // Apply HIGH_BIT_MASK only once at the end
    //matches  & !escaped(haystack) & HIGH_BIT_MASK

    let high_bit_mask = _mm512_set1_epi8(-1);

    _mm512_and_si512(matches, high_bit_mask)
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

    _mm512_set_epi64(e0, e1, e2, e3, e4, e5, e6, e7)
}

#[inline(always)]
unsafe fn get_512_zeros_iteration(matches: __m512i) -> ([i64; 8], __m512i) {
    let zeros = _mm512_lzcnt_epi64(matches);
    let zeros_index = _mm512_srli_epi64(zeros, 3);

    let mut list: [i64; 8] = [0; 8];
    _mm512_storeu_epi64(list.as_mut_ptr(), zeros_index);

    let one = _mm512_set1_epi64(1);
    let matches_minus_one = _mm512_sub_epi64(matches, one);
    let new_matches = _mm512_and_si512(matches, matches_minus_one);

    (list, new_matches)
}

#[inline(always)]
unsafe fn get_512_zeros(out: &mut Vec<u32>, offset: u32, matches: __m512i) {
    let (i0, matches) = get_512_zeros_iteration(matches);
    let (i1, matches) = get_512_zeros_iteration(matches);
    let (i2, matches) = get_512_zeros_iteration(matches);
    let (i3, matches) = get_512_zeros_iteration(matches);
    let (i4, matches) = get_512_zeros_iteration(matches);
    let (i5, matches) = get_512_zeros_iteration(matches);
    let (i6, matches) = get_512_zeros_iteration(matches);
    let (i7, _) = get_512_zeros_iteration(matches);

    out.reserve(8);

    println!("{:?}", i0);
    println!("{:?}", i1);
    println!("{:?}", i2);
    println!("{:?}", i3);
    println!("{:?}", i4);
    println!("{:?}", i5);
    println!("{:?}", i6);
    println!("{:?}", i7);

    for i in 0..8 {
        let mut j = offset as usize + i;
        out[j] = (i0[i] as u32) + offset;
        j += if i0[i] == 0 { 0 } else { 1 };

        out[j] = (i1[i] as u32) + offset;
        j += if i1[i] == 0 { 0 } else { 1 };

        out[j] = (i2[i] as u32) + offset;
        j += if i2[i] == 0 { 0 } else { 1 };

        out[j] = (i3[i] as u32) + offset;
        j += if i3[i] == 0 { 0 } else { 1 };

        out[j] = (i4[i] as u32) + offset;
        j += if i4[i] == 0 { 0 } else { 1 };

        out[j] = (i5[i] as u32) + offset;
        j += if i5[i] == 0 { 0 } else { 1 };

        out[j] = (i6[i] as u32) + offset;
        j += if i6[i] == 0 { 0 } else { 1 };

        out[j] = (i7[i] as u32) + offset;
        j += if i7[i] == 0 { 0 } else { 1 };
    }
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

        let matches = unsafe { structural_mask(word) };

        println!("{:?}", matches);
        unsafe { get_512_zeros(&mut out, i as u32, matches) };
        // while matches != 0 {
        //     //let byte_offset = matches.trailing_zeros() / (STEP as u32);
        //     let byte_offset = matches.trailing_zeros() >> 3;
        //     let index = byte_offset + i as u32;
        //     out.push(index);
        //     matches &= matches - 1;
        // }

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
    use super::*;
    #[test]
    fn test_comparison_x86_64() {
        let string = r#"<    div   >HEllo World <a href="link" class="\"my class\""> HERe  \</ a href="Fake link<span> Hello </span>"\>\<\a\></a><   /  div >"#;
        let indices = parse(string);
    }
}
