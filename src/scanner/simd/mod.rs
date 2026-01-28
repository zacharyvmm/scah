pub mod swar;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

pub trait SIMD {
    type RegisterSize: Copy;
    const BYTES: usize;

    //fn compare(haystack: Self::RegisterSize, needle: u8) -> u64;
    fn get_word(ptr: *const u8, offset: usize) -> Self::RegisterSize;
    fn get_word_aligned(buffer: &[u64], offset: usize) -> Self::RegisterSize;
    fn next_escape_and_terminal_code(haystack: u64) -> u64;
    fn escaped(haystack: Self::RegisterSize, next_is_escaped: u64) -> (u64, u64);

    fn buffer(input: &str) -> Vec<u8> {
        let mut buffer = input.as_bytes().to_vec();
        buffer.resize(buffer.len() + Self::BYTES, 0u8);
        buffer
    }

    fn structural_mask(haystack: Self::RegisterSize) -> u64;

    #[inline(always)]
    fn trailing_zeros(mask: u64) -> u32 {
        mask.trailing_zeros()
    }

    #[inline(always)]
    fn filter(mask: u64) -> u64 {
        mask
    }

    #[inline(always)]
    fn next_is_escaped(mask: u64) -> u64 {
        mask >> 63
    }
}
