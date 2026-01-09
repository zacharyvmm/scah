use super::super::fsm::Fsm;
use super::super::lexer::Lexer;
use crate::utils::Reader;

#[derive(PartialEq, Debug, Default, Clone, Copy)]
pub struct Save {
    // attributes: bool, // If your saving this has to be on
    pub inner_html: bool,
    pub text_content: bool,
}

impl Save {
    pub fn only_inner_html() -> Self {
        Self {
            inner_html: true,
            text_content: false,
        }
    }

    pub fn only_text_content() -> Self {
        Self {
            inner_html: false,
            text_content: true,
        }
    }

    pub fn all() -> Self {
        Self {
            inner_html: true,
            text_content: true,
        }
    }

    pub fn none() -> Self {
        Self {
            inner_html: false,
            text_content: false,
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
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

#[derive(PartialEq, Debug, Clone)]
pub struct SelectionPart<S> {
    pub source: S,
    pub kind: SelectionKind,

    pub parent: Option<usize>, // real index
    pub children: Vec<usize>,  // offset
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

impl<S> SelectionPart<S> {
    pub fn new(query: S, mode: SelectionKind) -> Self {
        Self {
            source: query,
            kind: mode,
            parent: None,
            children: Vec::new(),
        }
    }
}

impl<'query> SelectionPart<&'query str> {
    pub fn build(self) -> QuerySection<'query> {
        let reader = &mut Reader::new(self.source);
        let mut fsms = Vec::new();
        while let Some((combinator, element)) = Lexer::next(reader) {
            fsms.push(Fsm::new(combinator, element));
        }

        QuerySection {
            source: self.source,
            fsms: fsms.into_boxed_slice(),
            kind: self.kind,
            parent: self.parent,
            children: self.children.into_boxed_slice(),
        }
    }
}