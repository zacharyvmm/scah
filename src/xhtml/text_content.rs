use crate::utils::Reader;
use std::ops::{Range, RangeFrom};

#[derive(Debug, PartialEq)]
pub struct TextContent {
    pub(super) content: String,
    pub(super) text_start: Option<usize>,
    recording: bool,
}

impl TextContent {
    pub fn new(capacity: usize) -> Self {
        Self {
            content: String::with_capacity(capacity),
            text_start: None,
            recording: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn start_recording(&mut self) {
        self.recording = true;
    }

    pub fn stop_recording(&mut self) {
        self.recording = false;
    }

    pub fn set_start(&mut self, start: usize) {
        if !self.recording {
            return;
        }
        self.text_start = Some(start);
    }

    pub fn get_position(&self) -> usize {
        assert!(!self.content.is_empty());
        // BUG: the position is off by one
        self.content.len() - 2
    }

    pub fn push(&mut self, reader: &[u8], end_position: usize) -> Option<usize> {
        if !self.recording {
            return None;
        }
        // It has to be inside an element, so this is an impossible case other than at initialization
        let Some(start_position) = self.text_start else {
            unreachable!("Their has to be a start position set before pushing text content")
        };
        let text =
            unsafe { str::from_utf8_unchecked(&reader[start_position..end_position]) }.trim();

        // TODO: In browsers `\n` is ignored and multiple ` ` are tretead as one.
        // If the user wants the textcontent and innerhtml to be in format then I would need to filter the text
        // The only free things I can do is trim on both sides on the string

        self.text_start = None;

        if text.is_empty() {
            return None;
        }

        self.content.push_str(text);
        self.content.push(' ');
        Some(self.get_position())
    }

    pub fn slice(&self, range: Range<usize>) -> &str {
        &self.content[range]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_textcontent_record_when_needed() {
        let mut text = TextContent::new(11);
        let reader = b"Hello World";
        text.set_start(0);
        text.set_start(0);
        text.push(reader, 5);

        assert!(text.content.is_empty());
        text.start_recording();

        text.set_start(0);
        text.push(reader, 5);
        assert_eq!(text.content.trim(), "Hello".to_string());

        text.stop_recording();

        text.set_start(0);
        text.push(reader, 5);
        assert_eq!(text.content.trim(), "Hello".to_string());

        text.start_recording();

        text.set_start(0);
        text.push(reader, 5);
        assert_eq!(text.content.trim(), "Hello Hello".to_string());
    }
}
