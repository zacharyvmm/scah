const LOW_BIT_MASK: u64 = 0x0101010101010101;
const HIGH_BIT_MASK: u64 = 0x8080808080808080;
const ODD_BYTES: u64 = 0x0080008000800080;

const LAST_BYTE_ESCAPED: u64 = 0x80;

#[inline(always)]
fn compare(haystack: u64, needle: u8) -> u64 {
    let pattern = (needle as u64) * LOW_BIT_MASK;
    let comparison = haystack ^ pattern;

    comparison.wrapping_sub(LOW_BIT_MASK) & !comparison
}

fn escaped(haystack: u64) -> u64 {
    let backslashes = compare(haystack, b'\\') & HIGH_BIT_MASK;
    let maybe_escaped = backslashes << 8;

    let maybe_escaped_and_odd_bits = maybe_escaped | ODD_BYTES;
    let even_series_codes_and_odd_bits = maybe_escaped_and_odd_bits.wrapping_sub(backslashes);
    let escape_and_terminal_code = even_series_codes_and_odd_bits ^ ODD_BYTES;

    let escaped = escape_and_terminal_code ^ backslashes; // escaped character
    // let escape = escape_and_terminal_code & backslashes; // the `\` that escaped it
    escaped
}

fn structural_mask(haystack: u64) -> u64 {
    let matches = compare(haystack, b'<')
        | compare(haystack, b'>')
        | compare(haystack, b' ')
        | compare(haystack, b'"')
        | compare(haystack, b'\'')
        | compare(haystack, b'=')
        | compare(haystack, b'/')
        | compare(haystack, b'!');

    // Apply HIGH_BIT_MASK only once at the end
    matches & !escaped(haystack) & HIGH_BIT_MASK
}

// Assume that 1/8 of the bytes are structural
const RATIO_DENOMINATOR: usize = 8;

fn _parse(buffer: &[u8]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(buffer.len() / RATIO_DENOMINATOR);
    let ptr = buffer.as_ptr();

    // Iterate only up to original length
    let len = buffer.len() - 8;

    let mut i = 0;
    const STEP: usize = 8; // 64/8 = 8
    while i < len {
        let word = unsafe {
            let as_64_block = ptr.add(i) as *const u64;
            as_64_block.read_unaligned()
        };

        let mut matches = structural_mask(word);
        while matches != 0 {
            //let byte_offset = matches.trailing_zeros() / (STEP as u32);
            let byte_offset = matches.trailing_zeros() >> 3;
            let index = byte_offset + i as u32;
            out.push(index);
            matches &= matches - 1;
        }

        i += STEP;
    }

    out
}

fn buffer(input: &str) -> Vec<u8> {
    let mut buffer = input.as_bytes().to_vec();

    // Add 8 (64/8 = 8) null bytes to the end
    buffer.extend_from_slice(&[0u8; 8]);

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
    fn test_bulk_swar_indexing() {
        let string = "<div class=\"hello-world\" id=hello-world>Hello World</div>";
        let indices = parse(string);

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
    fn test_indexing_for_html_like_string() {
        let string = r#"<    div   >HEllo World <a href="link" class="\"my class\""> HERe  \</ a href="Fake link<span> Hello </span>"\>\<\a\></a><   /  div >"#;
        for (i, c) in string.as_bytes().iter().enumerate() {
            println!("{i}: {}", *c as char);
        }
        let indices = parse(string);
        let expected: Vec<u32> = vec![
            0, 1, 2, 3, 4, 8, 9, 10, 11, 17, 23, 24, 26, 31, 32, 37, 38, 44, 45, 50, 58, 59, 60,
            65, 66, 69, 70, 72, 77, 78, 83, 88, 93, 94, 100, 101, 102, 107, 108, 117, 118, 120,
            121, 122, 123, 124, 125, 126, 127, 131, 132,
        ];
        assert_eq!(indices, expected);
    }

    #[test]
    fn test_find_escaped_characters() {
        let string = b"\\ \\ \\ \\n";
        let num = u64::from_le_bytes(*string);
        let escaped = escaped(num);

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
        let escaped = escaped(num);

        let expected = &[0, 0x80, 0, 0x80, 0, 0, 0, 0x80];
        let expected = u64::from_le_bytes(*expected);

        assert_eq!(
            escaped, expected,
            "ERR:\nescaped:\t{:064b}\nexpected:\t{:064b}",
            escaped, expected
        );
    }
}
