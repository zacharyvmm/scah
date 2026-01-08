use super::super::fsm::Fsm;
use super::super::lexer::Lexer;
use crate::utils::Reader;

#[derive(PartialEq, Debug)]
pub struct Save {
    // attributes: bool, // If your saving this has to be on
    pub inner_html: bool,
    pub text_content: bool,
}

impl Default for Save {
    fn default() -> Self {
        Self {
            inner_html: false,
            text_content: false,
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum SelectionKind {
    First(Save),
    All(Save),
}

impl SelectionKind {
    pub fn save(&self) -> &Save {
        match self {
            Self::All(save) => save,
            Self::First(save) => save,
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct SelectionPart<'query> {
    pub source: &'query str,
    pub(crate) fsms: Vec<Fsm<'query>>,
    pub kind: SelectionKind,

    pub(crate) parent: Option<usize>, // real index
    pub(crate) children: Vec<usize>,  // offset
}

#[derive(Debug)]
pub struct QuerySection<'query> {
    pub source: &'query str,
    pub(crate) fsms: Box<[Fsm<'query>]>,
    pub kind: SelectionKind,

    pub(crate) parent: Option<usize>,  // real index
    pub(crate) children: Box<[usize]>, // offset
}

impl<'query> QuerySection<'query> {
    pub fn len(&self) -> usize {
        self.fsms.len()
    }
}

impl<'query> SelectionPart<'query> {
    pub fn new(query: &'query str, mode: SelectionKind) -> Self {
        let reader = &mut Reader::new(query);
        let mut selection = Self {
            source: query,
            fsms: Vec::new(),
            kind: mode,
            parent: None,
            children: Vec::new(),
        };

        while let Some((combinator, element)) = Lexer::next(reader) {
            selection.fsms.push(Fsm::new(combinator, element));
        }

        selection
    }

    pub fn len(&self) -> usize {
        self.fsms.len()
    }

    pub fn build(self) -> QuerySection<'query> {
        QuerySection {
            source: self.source,
            fsms: self.fsms.into_boxed_slice(),
            kind: self.kind,
            parent: self.parent,
            children: self.children.into_boxed_slice(),
        }
    }
}
