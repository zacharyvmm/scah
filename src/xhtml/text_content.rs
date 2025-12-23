use crate::utils::Reader;
use std::ops::RangeFrom;

#[derive(Debug)]
pub struct TextContent<'html> {
    pub(super) list: Vec<&'html str>,
    pub(super) text_start: Option<usize>,
}

impl<'html> TextContent<'html> {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            text_start: None,
        }
    }

    pub fn set_start(&mut self, start: usize) {
        self.text_start = Some(start);
    }

    pub fn get_position(&self) -> usize {
        assert!(!self.list.is_empty());
        // BUG: the position is off by one
        self.list.len() - 1
    }

    pub fn push(&mut self, reader: &Reader<'html>, end_position: usize) -> Option<usize> {
        // It has to be inside an element, so this is an impossible case other than at initialization
        let Some(start_position) = self.text_start else {
            unreachable!("Their has to be a start position set before pushing text content")
        };
        let text = reader.slice(start_position..end_position).trim();

        // TODO: In browsers `\n` is ignored and multiple ` ` are tretead as one.
        // If the user wants the textcontent and innerhtml to be in format then I would need to filter the text
        // The only free things I can do is trim on both sides on the string

        self.text_start = None;

        if text.len() > 0 || text.trim().len() > 0 {
            self.list.push(text.trim());
            return Some(self.get_position());
        }

        None
    }

    // It's assumed that you want from a start point to the current end of the text content list
    pub fn join(&self, range: RangeFrom<usize>) -> String {
        self.list[range].join(" ")
    }
}
