use super::selectors::{SelectorQuery, SelectorQueryKind};
use crate::xhtml::element::element::XHtmlElement;
use std::ops::Range;

type ArraySize = usize;
//type ArraySize = u16; // max size of 65535

#[derive(Debug, PartialEq)]
pub struct BodyContent<'a> {
    pub element: XHtmlElement<'a>,
    pub text_content: Option<Range<ArraySize>>,
    pub inner_html: Option<Range<ArraySize>>,
}

// usize is the index of in the elements list
// This format is easier to deal with collisions, thus I don't need to copy the information

type SelectElement = Option<ArraySize>;
type SelectCollection = Vec<ArraySize>;
type SelectMultiPointCollection = Vec<Vec<ArraySize>>;

#[derive(Debug, PartialEq)]
pub enum Select<'query> {
    All(&'query str, SelectCollection),
    One(&'query str, SelectElement),
}

impl<'query> Select<'query> {
    pub(crate) fn push(&mut self, content: ArraySize) -> () {
        match self {
            Select::All(_, vec) => vec.push(content),
            Select::One(query, Option::None) => *self = Select::One(*query, Some(content)),
            Select::One(_, Some(_)) => {
                panic!("Selection set to a single element, but tried to append an element")
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SelectionMap<'query, 'html> {
    pub elements: Vec<BodyContent<'html>>,
    pub mappings: Vec<Select<'query>>,
}

impl<'query, 'html> SelectionMap<'query, 'html> {
    pub(crate) fn new(queries: &Vec<SelectorQuery<'query>>) -> Self {
        let mut map: Self = Self {
            elements: Vec::new(),
            mappings: Vec::with_capacity(queries.len()),
        };
        for i in 0..queries.len() {
            map.mappings.push(match queries[i].kind {
                SelectorQueryKind::All => Select::All(queries[i].query, Vec::new()),
                SelectorQueryKind::First => Select::One(queries[i].query, None),
            });
        }

        return map;
    }

    pub(crate) fn add_element(&mut self, content: BodyContent<'html>) -> ArraySize {
        self.elements.push(content);

        return (self.elements.len() - 1) as ArraySize;
    }

    pub(crate) fn push(&mut self, index: usize, element_index: ArraySize) -> () {
        assert!(index < self.mappings.len());

        assert!(
            element_index < self.elements.len() as ArraySize,
            "{} < {}",
            element_index,
            self.elements.len()
        );
        self.mappings[index].push(element_index);
    }
}
