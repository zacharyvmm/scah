use super::simd::{SIMD, swar};

#[cfg(target_arch = "x86_64")]
use super::simd::x86_64;

pub struct Scanner {
    next_is_escaped: u64,
}

#[derive(PartialEq, Debug)]
pub enum CPUID {
    AVX512BW,
    Other,
}

impl CPUID {
    #[cfg(target_arch = "x86_64")]
    pub fn detect_x86() -> Self {
        if is_x86_feature_detected!("avx512bw") {
            Self::AVX512BW
        } else {
            Self::Other
        }
    }
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        return Self::detect_x86();

        return Self::Other;
    }
}

impl Scanner {
    pub fn new() -> Self {
        Self { next_is_escaped: 0 }
    }
    // Create a buffer
    pub fn open_file() {
        todo!()
    }
    pub fn scan<T: SIMD>(&mut self, buffer: &[u8], len: usize) -> Vec<u32> {
        const RATIO_DENOMINATOR: usize = 8;
        let mut out: Vec<u32> = Vec::with_capacity(buffer.len() / RATIO_DENOMINATOR);
        let ptr = buffer.as_ptr();

        let mut i = 0;
        while i < len {
            let word = T::get_word(ptr, i);

            let mut matches = {
                let mask = T::structural_mask(word);
                let (escaped, escape) = T::escaped(word, self.next_is_escaped);
                self.next_is_escaped = T::next_is_escaped(escape);

                T::filter(mask & !escaped)
            };
            while matches != 0 {
                let byte_offset = T::trailing_zeros(matches);
                let index = byte_offset + i as u32;
                out.push(index);
                matches &= matches - 1;
            }

            i += T::BYTES;
        }

        out
    }

    pub fn scan_aligned<T: SIMD>(&mut self, buffer: &[u64], len: usize) -> Vec<u32> {
        const RATIO_DENOMINATOR: usize = 8;
        let mut out: Vec<u32> = Vec::with_capacity(buffer.len() / RATIO_DENOMINATOR);

        let mut i = 0;
        while i < len {
            let word = T::get_word_aligned(buffer, i / T::BYTES);

            let mut matches = {
                let mask = T::structural_mask(word);
                let (escaped, escape) = T::escaped(word, self.next_is_escaped);
                self.next_is_escaped = T::next_is_escaped(escape);

                T::filter(mask & !escaped)
            };
            while matches != 0 {
                let byte_offset = T::trailing_zeros(matches);
                let index = byte_offset + i as u32;
                out.push(index);
                matches &= matches - 1;
            }

            i += T::BYTES;
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_indexing_for_html_like_string() {
        let string = r#"<    div   >HEllo World <a href="link" class="\"my class\""> HERe  \</ a href="Fake link<span> Hello </span>"\>\<\a\></a><   /  div >"#;

        let mut scanner = Scanner::new();

        for (i, c) in string.as_bytes().iter().enumerate() {
            println!("{i}: {}", *c as char);
        }
        let buffer = swar::SWAR::buffer(string);
        let indices =
            scanner.scan::<swar::SWAR>(buffer.as_slice(), buffer.len() - swar::SWAR::BYTES);
        let expected: Vec<u32> = vec![
            0, 1, 2, 3, 4, 8, 9, 10, 11, 17, 23, 24, 26, 31, 32, 37, 38, 44, 45, 50, 58, 59, 60,
            65, 66, 69, 70, 72, 77, 78, 83, 88, 93, 94, 100, 101, 102, 107, 108, 117, 118, 120,
            121, 122, 123, 124, 125, 126, 127, 131, 132,
        ];
        assert_eq!(indices, expected);

        let buffer = x86_64::SIMD512::buffer(string);
        let indices = scanner
            .scan::<x86_64::SIMD512>(buffer.as_slice(), buffer.len() - x86_64::SIMD512::BYTES);
        assert_eq!(indices, expected);
    }

    #[test]
    fn test_dispatch() {
        assert_eq!(CPUID::detect(), CPUID::AVX512BW);
    }

    #[test]
    fn test_next_is_escaped_swar() {
        let mut next_is_escaped = 0;
        let string = r#"<    div   >HEllo World <a href="link" class="\"my class\""> HERe  \</ a href="Fake link<span> Hello </span>"\>\<\a\></a><   /  div >"#;
        let buffer = swar::SWAR::buffer(string);

        let expected = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0, 0, 0];

        for i in 0..17 {
            let word = swar::SWAR::get_word(buffer.as_ptr(), i * 8);
            println!("{i}: {}", str::from_utf8(&word.to_le_bytes()).unwrap());
            let (escaped, escape) = swar::SWAR::escaped(word, next_is_escaped);
            next_is_escaped = swar::SWAR::next_is_escaped(escape);
            assert_eq!(
                next_is_escaped,
                expected[i],
                "\n'{}'\n{word:064b}\n{escaped:064b}",
                str::from_utf8(&word.to_le_bytes()).unwrap()
            );
        }
    }

    #[test]
    fn test_no_allocation_swar() {
        let string = r#"<    div   >HEllo World <a href="link" class="\"my class\""> HERe  \</ a href="Fake link<span> Hello </span>"\>\<\a\></a><   /  div >"#;
        let (before, bytes, after) = unsafe { string.as_bytes().align_to::<u64>() };
        println!(
            "BEFORE: {}, ALIGNED: {}, AFTER: {}",
            before.len(),
            bytes.len() * 8,
            after.len()
        );

        let buf_before = {
            let mut list = [0u8; 8];
            list[..before.len()].copy_from_slice(before);
            list
        };

        let buf_after = {
            let mut list = [0u8; 8];
            list[..after.len()].copy_from_slice(after);
            list
        };

        let mut scanner = Scanner::new();
        let mut no_copy_indices: Vec<u32> = scanner.scan::<swar::SWAR>(&buf_before, 8);
        let mut len = before.len() as u32;

        let indices = scanner
            .scan_aligned::<swar::SWAR>(bytes, bytes.len() * 8)
            .into_iter()
            .map(|i| i + len);

        no_copy_indices.extend(indices);
        len += (bytes.len() * 8) as u32;

        let aindices = scanner
            .scan::<swar::SWAR>(&buf_after, 8)
            .into_iter()
            .map(|i| i + len);

        no_copy_indices.extend(aindices);

        let buffer = swar::SWAR::buffer(string);
        let buf_indices =
            Scanner::new().scan::<swar::SWAR>(buffer.as_slice(), buffer.len() - swar::SWAR::BYTES);

        assert_eq!(no_copy_indices, buf_indices);
    }

    #[test]
    fn test_no_allocation_simd() {
        let string = r#"<    div   >HEllo World <a href="link" class="\"my class\""> HERe  \</ a href="Fake link<span> Hello </span>"\>\<\a\></a><   /  div >"#;
        let (before, bytes, after) = unsafe { string.as_bytes().align_to::<u64>() };
        println!(
            "BEFORE: {}, ALIGNED: {}, AFTER: {}",
            before.len(),
            bytes.len() * 8,
            after.len()
        );

        let buf_before = {
            let mut list = [0u8; 8];
            list[..before.len()].copy_from_slice(before);
            list
        };

        let buf_after = {
            let mut list = [0u8; 8];
            list[..after.len()].copy_from_slice(after);
            list
        };

        let mut scanner = Scanner::new();
    
        let mut no_copy_indices: Vec<u32> = scanner.scan::<swar::SWAR>(&buf_before, 8);
        println!("BEfORE: {:?}", no_copy_indices);
        let mut len = before.len() as u32;

        let mut indices: Vec<u32> = scanner
            .scan_aligned::<x86_64::SIMD512>(bytes, bytes.len() * 8)
            .iter()
            .map(|i| *i + len)
            .collect();
        len += (bytes.len() * 8) as u32;
        println!("ALIGNED: {:?}", indices);

        let mut aindices = scanner
            .scan::<swar::SWAR>(&buf_after, 8)
            .iter()
            .map(|i| *i + len)
            .collect();

        println!("AFTER: {:?}", aindices);

        no_copy_indices.append(&mut indices);
        no_copy_indices.append(&mut aindices);

        let buffer = swar::SWAR::buffer(string);
        let buf_indices =
            Scanner::new().scan::<swar::SWAR>(buffer.as_slice(), buffer.len() - swar::SWAR::BYTES);

        let buffer = x86_64::SIMD512::buffer(string);
        let buf_indices_simd = Scanner::new()
            .scan::<x86_64::SIMD512>(buffer.as_slice(), buffer.len() - x86_64::SIMD512::BYTES);

        assert_eq!(buf_indices_simd, buf_indices);
        assert_eq!(no_copy_indices, buf_indices);
    }
}
