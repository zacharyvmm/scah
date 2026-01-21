mod swar;

#[cfg(target_arch = "x86_64")]
mod x86_64;

trait SIMD {
    type RegisterSize: Copy;
    const BYTES:usize;


    fn compare(haystack: Self::RegisterSize, needle: u8) -> u64;   
    fn get_word(ptr: *const u8, offset: usize) -> Self::RegisterSize;

    fn buffer(input: &str) -> Vec<u8> {
        let mut buffer = input.as_bytes().to_vec();
        buffer.resize(buffer.len() + Self::BYTES, 0u8);
        buffer
    }

    fn escaped(haystack: Self::RegisterSize) -> u64;

    fn structural_mask(haystack: Self::RegisterSize) -> u64 {
        let matches = Self::compare(haystack, b'<')
            | Self::compare(haystack, b'>')
            | Self::compare(haystack, b' ')
            | Self::compare(haystack, b'"')
            | Self::compare(haystack, b'\'')
            | Self::compare(haystack, b'=')
            | Self::compare(haystack, b'/')
            | Self::compare(haystack, b'!');

        matches & !Self::escaped(haystack)
    }
 
    #[inline(always)]
    fn trailing_zeros(mask: u64) -> u32 {
        mask.trailing_zeros()
    }

    #[inline(always)]
    fn filter(mask:u64) -> u64 {
        mask
    }

    fn _parse(buffer: &[u8]) -> Vec<u32> {
        const RATIO_DENOMINATOR: usize = 8;
        let mut out: Vec<u32> = Vec::with_capacity(buffer.len() / RATIO_DENOMINATOR);
        let ptr = buffer.as_ptr();

        // Iterate only up to original length
        let len = buffer.len() - Self::BYTES;

        let mut i = 0;
        while i < len {
            let word = Self::get_word(ptr, i);

            let mut matches = {
                let mask = Self::structural_mask(word);
                Self::filter(mask)
            };
            while matches != 0 {
                let byte_offset = Self::trailing_zeros(matches);
                let index = byte_offset + i as u32;
                out.push(index);
                matches &= matches - 1;
            }

            i += Self::BYTES;
        }

        out
    }

    fn parse(input: &str) -> Vec<u32> {
        let buffer = Self::buffer(input);
        let indices = Self::_parse(&buffer);
        indices
    }
}