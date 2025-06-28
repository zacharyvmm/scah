use super::selectors::{SelectorQuery, SelectorQueryKind};
use crate::xhtml::element::element::XHtmlElement;
use std::ops::Range;

#[derive(Debug, PartialEq)]
pub struct BodyContent<'a> {
    pub(crate) element: XHtmlElement<'a>,
    pub(crate) text_content: Option<Range<usize>>,
    pub(crate) inner_html: Option<Range<usize>>,
}

// usize is the index of in the elements list
// This format is easier to deal with collisions, thus I don't need to copy the information
type SelectCollection = Vec<usize>;
type SelectElement = Option<usize>;

#[derive(Debug, PartialEq)]
pub enum Select {
    All(SelectCollection),
    One(SelectElement),
}

impl Select {
    pub(crate) fn push(&mut self, content: usize) -> () {
        match self {
            Select::All(vec) => vec.push(content),
            Select::One(Option::None) => *self = Select::One(Some(content)),
            Select::One(Some(_)) => {
                panic!("Selection set to a single element, but tried to append an element")
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SelectionMap<'query, 'html> {
    pub elements: Vec<BodyContent<'html>>,
    pub mappings: Vec<(&'query str, Select)>,
}

impl<'query, 'html> SelectionMap<'query, 'html> {
    pub(crate) fn new(queries: &Vec<SelectorQuery<'query>>) -> Self {
        let mut map: Self = Self {
            elements: Vec::new(),
            mappings: Vec::with_capacity(queries.len()),
        };
        for i in 0..queries.len() {
            let mapping: (&'query str, Select) = (
                queries[i].query,
                match queries[i].kind {
                    SelectorQueryKind::All => Select::All(Vec::new()),
                    SelectorQueryKind::First => Select::One(None),
                },
            );
            map.mappings.push(mapping);
        }

        return map;
    }

    pub(crate) fn add_element(&mut self, content: BodyContent<'html>) -> usize {
        self.elements.push(content);

        return self.elements.len() - 1;
    }

    pub(crate) fn push(&mut self, index: usize, element_index: usize) -> () {
        assert!(index < self.mappings.len());

        assert!(
            element_index < self.elements.len(),
            "{} < {}",
            element_index,
            self.elements.len()
        );
        self.mappings[index].1.push(element_index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_() {}
}
