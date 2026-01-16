const LOW_BIT_MASK: u64 = 0x0101010101010101;
const HIGH_BIT_MASK: u64 = 0x8080808080808080;

#[inline(always)]
fn compare(haystack: u64, needle: u8) -> u64 {
    let pattern = (needle as u64) * LOW_BIT_MASK;
    let comparison = haystack ^ pattern;

    comparison.wrapping_sub(LOW_BIT_MASK) & !comparison
}

fn structural_mask(haystack: u64) -> u64 {
    let matches = compare(haystack, b'<')
        | compare(haystack, b'>')
        | compare(haystack, b' ')
        | compare(haystack, b'"')
        | compare(haystack, b'\'')
        | compare(haystack, b'=')
        | compare(haystack, b'/');

    // Apply HIGH_BIT_MASK only once at the end
    matches & HIGH_BIT_MASK
}

// Assume that 1/8
const RATIO_DENOMINATOR: usize = 8;

fn _parse(buffer: &[u8]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(buffer.len() / RATIO_DENOMINATOR);
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
            //let byte_offset = matches.trailing_zeros() / 8;
            let byte_offset = matches.trailing_zeros() >> 3;
            let index = byte_offset + i as u32;
            out.push(index);
            matches &= matches - 1;
        }

        i += 8;
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
