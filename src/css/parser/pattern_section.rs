use super::query_tokenizer::PatternStep;
use crate::utils::reader::Reader;

#[derive(PartialEq, Debug)]
pub enum Mode {
    First,
    All,
}

#[derive(PartialEq, Debug)]
pub struct PatternSection<'a> {
    pub(crate) pattern: Vec<PatternStep<'a>>,
    pub(super) kind: Mode,

    pub(super) parent: Option<usize>, // real index
    pub(super) children: Vec<usize>,  // offset
}

impl<'a> PatternSection<'a> {
    pub fn new(reader: &mut Reader<'a>, mode: Mode) -> Self {
        let mut selection = Self {
            pattern: Vec::new(),
            kind: mode,
            parent: None,
            children: Vec::new(),
        };

        while let Some(token) = PatternStep::next(reader, selection.pattern.last()) {
            selection.pattern.push(token);
        }
        selection.pattern.push(PatternStep::Save);

        return selection;
    }
}
