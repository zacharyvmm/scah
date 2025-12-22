use std::ops::{Range, RangeInclusive};
use crate::utils::Reader;

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
        assert!(self.list.len() > 0);
        // BUG: the position is off by one
        self.list.len()// - 1
    }

    pub fn push(&mut self, reader: &Reader<'html>, end_position: usize) {
        // It has to be inside an element, so this is an impossible case other than at initialization
        if let Some(start_position) = self.text_start {
            let text = reader.slice(start_position..end_position).trim();

            // TODO: In browsers `\n` is ignored and multiple ` ` are tretead as one.
            // If the user wants the textcontent and innerhtml to be in format then I would need to filter the text
            // The only free things I can do is trim on both sides on the string
            if text.len() > 0 {
                self.list.push(text);
            }

            self.text_start = None;
        }
        //self.text_start = reader.get_position();
    }

    // TODO: should be `RangeInclusive`
    pub fn join(&self, range: Range<usize>) -> String {
        println!("Joining text content from {:?}: {:#?}", range, self.list);
        self.list[range].join(" ")
    }
}
