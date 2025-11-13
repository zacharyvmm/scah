use crate::XHtmlElement;
use std::ops::Range;

#[derive(PartialEq, Debug)]
pub enum ContentRange {
    Empty,
    StartPoint(usize),
    Complete(Range<usize>),
}

impl ContentRange {
    fn new(start_position: Option<usize>) -> Self {
        match start_position {
            Some(start) => Self::StartPoint(start),
            None => Self::Empty,
        }
    }
}

// This is essentially a Selection DOM or Selection Object Model
#[derive(PartialEq, Debug)]
pub struct MatchTree<'html> {
    pub(crate) list: Vec<Node<'html>>,
}

#[derive(PartialEq, Debug)]
pub struct Node<'html> {
    pub value: XHtmlElement<'html>,
    pub inner_html: ContentRange,
    pub text_content: ContentRange,

    pub children: Vec<usize>,
}

impl<'html> Node<'html> {
    fn new(
        element: XHtmlElement<'html>,
        start_position_inner_html: Option<usize>,
        start_position_text_content: Option<usize>,
    ) -> Self {
        Self {
            value: element,
            inner_html: ContentRange::new(start_position_inner_html),
            text_content: ContentRange::new(start_position_text_content),
            children: Vec::new(),
        }
    }
}

// A fake save element is added as the first element to handle roots
const ROOT_POSITION: usize = 0;

// This is a fake element inserted into the first element possition to handle the root positions
const ROOT_NODE: Node = Node {
    value: XHtmlElement {
        name: "root",
        class: None,
        id: None,
        attributes: Vec::new(),
    },
    children: vec![],
    inner_html: ContentRange::Empty,
    text_content: ContentRange::Empty,
};

impl<'html> MatchTree<'html> {
    pub fn new() -> Self {
        Self {
            list: Vec::from([ROOT_NODE]),
        }
    }

    pub fn push(
        &mut self,
        parent: usize,
        element: XHtmlElement<'html>,
        start_position_inner_html: Option<usize>,
        start_position_text_content: Option<usize>,
    ) -> usize {
        let last_index = self.list.len();
        self.list.push(Node::new(
            element,
            start_position_inner_html,
            start_position_text_content,
        ));
        self.list[parent].children.push(last_index);
        return last_index;
    }

    pub fn set_content(
        &mut self,
        position: usize,
        end_position_inner_html: usize,
        end_position_text_content: usize,
    ) {
        assert!(
            matches!(self.list[position].inner_html, ContentRange::Complete(..)),
            "You cannot add another point when the start and end points are already set."
        );
        assert!(
            matches!(self.list[position].text_content, ContentRange::Complete(..)),
            "You cannot add another point when the start and end points are already set."
        );

        if let ContentRange::StartPoint(start) = self.list[position].inner_html {
            self.list[position].inner_html = ContentRange::Complete(start..end_position_inner_html);
        }

        if let ContentRange::StartPoint(start) = self.list[position].text_content {
            self.list[position].text_content =
                ContentRange::Complete(start..end_position_text_content);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::Reader;

    use super::*;

    #[test]
    fn test_build_tree() {
        /*
         * root -> a.class --> span#id --> p
         *      |          \-> p
         *      \-> div
         */
        let mut tree = MatchTree::new();

        // SETUP Elements
        let first = XHtmlElement::from(&mut Reader::new("a class=\"class\""));
        let second = XHtmlElement::from(&mut Reader::new("span id=\"id\""));
        let second_extended = XHtmlElement::from(&mut Reader::new("p"));
        let second_alternate = XHtmlElement::from(&mut Reader::new("p"));
        let first_alternate = XHtmlElement::from(&mut Reader::new("div"));

        let mut index = tree.push(0, first, None, None);
        _ = tree.push(0, first_alternate, None, None);
        _ = tree.push(index, second_alternate, None, None);
        index = tree.push(index, second, None, None);
        _ = tree.push(index, second_extended, None, None);

        assert_eq!(
            tree.list,
            vec![
                Node {
                    value: XHtmlElement {
                        name: "root",
                        class: None,
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![1, 2],
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                },
                Node {
                    value: XHtmlElement {
                        name: "a",
                        class: Some("class"),
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![3, 4],
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                },
                Node {
                    value: XHtmlElement {
                        name: "div",
                        class: None,
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![],
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                },
                Node {
                    value: XHtmlElement {
                        name: "p",
                        class: None,
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![],
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                },
                Node {
                    value: XHtmlElement {
                        name: "span",
                        class: None,
                        id: Some("id"),
                        attributes: Vec::new()
                    },
                    children: vec![5],
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                },
                Node {
                    value: XHtmlElement {
                        name: "p",
                        class: None,
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![],
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                },
            ]
        )
    }
}
