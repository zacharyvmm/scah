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
enum Select {
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

pub struct SelectionMap<'query, 'html> {
    elements: Vec<BodyContent<'html>>,
    mappings: Vec<(&'query str, Select)>,
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

    pub(crate) fn add_element(&mut self, element: XHtmlElement<'html>) -> usize {
        self.elements.push(BodyContent {
            element: element,
            text_content: None,
            inner_html: None,
        });

        return self.elements.len();
    }

    pub(crate) fn push(
        &mut self,
        index: usize,
        element_index: usize,
        text_content: Option<Range<usize>>,
        inner_html: Option<Range<usize>>,
    ) -> () {
        if index >= self.mappings.len() {
            return;
        }

        if element_index >= self.elements.len() {
            return;
        }

        let content = &mut self.elements[element_index];
        content.text_content = text_content;
        content.inner_html = inner_html;

        self.mappings[index].1.push(element_index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_() {}
}
