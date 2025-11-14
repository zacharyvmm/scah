use super::super::fsm::Fsm;
use super::super::lexer::Lexer;
use crate::utils::Reader;

#[derive(PartialEq, Debug)]
pub struct Save {
    // attributes: bool, // If your saving this has to be on
    pub inner_html: bool,
    pub text_content: bool,
}

#[derive(PartialEq, Debug)]
pub enum SelectionKind {
    First(Save),
    All(Save),
}

#[derive(PartialEq, Debug)]
pub struct SelectionPart<'a> {
    pub(crate) fsms: Vec<Fsm<'a>>,
    pub descendants: Vec<usize>,
    pub(crate) kind: SelectionKind,

    pub(crate) parent: Option<usize>, // real index
    pub(crate) children: Vec<usize>,  // offset
}

impl<'a> SelectionPart<'a> {
    pub fn new(input: &'a str, mode: SelectionKind) -> Self {
        let reader = &mut Reader::new(input);
        let mut selection = Self {
            fsms: Vec::new(),
            descendants: Vec::new(),
            kind: mode,
            parent: None,
            children: Vec::new(),
        };

        while let Some((combinator, element)) = Lexer::next(reader) {
            selection.fsms.push(Fsm::new(combinator, element));
        }

        return selection;
    }

    pub fn len(&self) -> usize {
        return self.fsms.len();
    }
}
