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
    pub fn scan<T: SIMD>(&mut self, input: &str) -> Vec<u32> {
        let buffer = T::buffer(input);

        const RATIO_DENOMINATOR: usize = 8;
        let mut out: Vec<u32> = Vec::with_capacity(buffer.len() / RATIO_DENOMINATOR);
        let ptr = buffer.as_ptr();

        // Iterate only up to original length
        let len = buffer.len() - T::BYTES;

        let mut i = 0;
        while i < len {
            let word = T::get_word(ptr, i);

            let mut matches = {
                let mask = T::structural_mask(word);
                //println!("NExt Escaped: {:064b}", self.next_is_escaped);
                let escaped = T::escaped(word, self.next_is_escaped);
                self.next_is_escaped = T::next_is_escaped(escaped);
                //println!("NExt Escaped: {:064b}", self.next_is_escaped);

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
        let indices = scanner.scan::<swar::SWAR>(string);
        let expected: Vec<u32> = vec![
            0, 1, 2, 3, 4, 8, 9, 10, 11, 17, 23, 24, 26, 31, 32, 37, 38, 44, 45, 50, 58, 59, 60,
            65, 66, 69, 70, 72, 77, 78, 83, 88, 93, 94, 100, 101, 102, 107, 108, 117, 118, 120,
            121, 122, 123, 124, 125, 126, 127, 131, 132,
        ];
        assert_eq!(indices, expected);

        let indices = scanner.scan::<x86_64::SIMD512>(string);
        assert_eq!(indices, expected);
    }

    #[test]
    fn test_dispatch() {
        assert_eq!(CPUID::detect(), CPUID::AVX512BW);
    }
}
