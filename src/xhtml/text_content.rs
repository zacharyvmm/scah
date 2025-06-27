use std::ops::Range;

use crate::utils::reader::Reader;

pub struct TextContent<'html> {
    list: Vec<&'html str>,
    text_start: usize,
}

impl<'html> TextContent<'html> {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            text_start: 0,
        }
    }

    pub fn reset_start(&mut self) {
        self.text_start = 0;
    }

    pub fn push(&mut self, reader: &Reader<'html>) {
        // It has to be inside an element, so this is an impossible case other than at initialization
        if self.text_start == 0 {
            self.list
                .push(reader.slice(self.text_start..reader.get_position()));
        }
        self.text_start = reader.get_position();
    }

    pub fn concat(&self, range: Range<usize>) {
        self.list[range].concat();
    }
}
