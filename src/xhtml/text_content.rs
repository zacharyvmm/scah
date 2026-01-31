use crate::utils::Reader;
use std::ops::Range;

#[derive(Debug, PartialEq)]
pub struct TextContent {
    pub(super) content: Vec<u8>,
    pub(super) text_start: Option<usize>,
}

impl TextContent {
    pub fn new() -> Self {
        Self {
            content: Vec::new(),
            text_start: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            content: Vec::with_capacity(capacity),
            text_start: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn set_start(&mut self, start: usize) {
        self.text_start = Some(start);
    }

    pub fn get_position(&self) -> usize {
        assert!(!self.content.is_empty());
        // BUG: the position is off by one
        self.content.len() - 2
    }

    pub fn push<'html>(&mut self, reader: &Reader<'html>, end_position: usize) -> Option<usize> {
        // It has to be inside an element, so this is an impossible case other than at initialization
        let Some(start_position) = self.text_start else {
            unreachable!("Their has to be a start position set before pushing text content")
        };
        let text = reader.slice(start_position..end_position).trim();

        // TODO: In browsers `\n` is ignored and multiple ` ` are tretead as one.
        // If the user wants the textcontent and innerhtml to be in format then I would need to filter the text
        // The only free things I can do is trim on both sides on the string

        self.text_start = None;

        if text.is_empty() {
            return None;
        }

        self.content.extend_from_slice(text.as_bytes());
        self.content.push(b' ');
        Some(self.get_position())
    }

    // It's assumed that you want from a start point to the current end of the text content list
    pub fn slice(&self, range: Range<usize>) -> &str {
        unsafe { str::from_utf8_unchecked(&self.content[range]) }
    }

    pub fn data(self) -> Vec<u8> {
        self.content
    }

    pub fn to_string(self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_textcontent_record_when_needed() {
        let mut content = TextContent::new();
        let reader = Reader::new("Hello World");
        content.set_start(0);
        content.set_start(0);
        content.push(&reader, 5);

        assert!(content.content.is_empty());
        content.start_recording();

        content.set_start(0);
        content.push(&reader, 5);
        assert_eq!(content.content.trim(), "Hello".to_string());

        content.stop_recording();

        content.set_start(0);
        content.push(&reader, 5);
        assert_eq!(content.content.trim(), "Hello".to_string());

        content.start_recording();

        content.set_start(0);
        content.push(&reader, 5);
        assert_eq!(content.content.trim(), "Hello Hello".to_string());
    }
}
