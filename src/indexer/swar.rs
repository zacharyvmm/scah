fn mask(haystack: u64, needle: u8) -> u64 {
    const LOW_BIT_MASK: u64 = 0x0101010101010101;
    const HIGH_BIT_MASK: u64 = 0x8080808080808080;

    let mask = (needle as u64) * LOW_BIT_MASK;

    let comparison = haystack ^ mask;

    let potential_matches = comparison.wrapping_sub(LOW_BIT_MASK) & !comparison;

    let only_high_bit_match = potential_matches & HIGH_BIT_MASK;

    only_high_bit_match
}

fn structural_mask(haystack: u64) -> u64 {
    mask(haystack, b'<')
        | mask(haystack, b'>')
        | mask(haystack, b' ')
        | mask(haystack, b'"')
        | mask(haystack, b'\'')
        | mask(haystack, b'=')
        | mask(haystack, b'/')
}

fn _parse(buffer: &[u8]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let ptr = buffer.as_ptr();
    // Iterate only up to original length
    let len = buffer.len() - 8;

    let mut i = 0;
    while i < len {
        let word = unsafe {
            let as_64_block = ptr.add(i) as *const u64;
            as_64_block.read_unaligned()
        };

        let mut matches = structural_mask(word);
        while matches != 0 {
            let index = (matches.trailing_zeros() / 8) + i as u32;
            out.push(index);
            matches &= matches - 1;
        }

        i += 8;
    }

    out
}

fn buffer(input: &str) -> Vec<u8> {
    let mut buffer = input.as_bytes().to_vec();

    buffer.extend_from_slice(&[0u8; 8]);

    buffer
}

fn parse(input: &str) -> Vec<u32> {
    let buffer = buffer(input);
    let indices = _parse(&buffer);
    indices
}

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
}
