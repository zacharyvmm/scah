use std::ops::Range;

use crate::utils::reader::Reader;

#[derive(Debug)]
pub struct TextContent<'html> {
    pub(super) list: Vec<&'html str>,
    pub(super) text_start: usize,
}

impl<'html> TextContent<'html> {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            text_start: 0,
        }
    }

    pub fn set_start(&mut self, start: usize) {
        self.text_start = start;
    }

    pub fn get_position(&self) -> usize {
        assert_ne!(self.list.len(), 0);
        self.list.len()
    }

    pub fn push(&mut self, reader: &Reader<'html>, end_position: usize) {
        // It has to be inside an element, so this is an impossible case other than at initialization
        if self.text_start != 0 {
            let text = reader.slice(self.text_start..end_position).trim();

            // TODO: In browsers `\n` is ignored and multiple ` ` are tretead as one.
            // If the user wants the textcontent and innerhtml to be in format then I would need to filter the text
            // The only free things I can do is trim on both sides on the string
            if text.len() > 0 {
                self.list.push(text);
            }

            self.text_start = 0;
        }
        //self.text_start = reader.get_position();
    }

    pub fn join(&self, range: Range<usize>) -> String {
        self.list[range].join(" ")
    }
}
