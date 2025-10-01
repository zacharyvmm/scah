use crate::XHtmlElement;
use crate::css::fsm_session::Element;

#[derive(Default)]
pub struct Tree<'html> {
    list: Vec<Node<'html>>,
    root: usize,
}

pub struct Node<'html> {
    pub value: XHtmlElement<'html>, // for the rust lib it's better UX to have a reference instead of a index
    pub inner_html: Option<&'html str>,
    pub text_content: Option<&'html str>, // After concat evaluation

    pub children: Vec<usize>,
    pub parent: usize,
}

impl<'html> Node<'html> {
    fn new(parent: usize, element: XHtmlElement<'html>) -> Self {
        Self {
            value: element,
            inner_html: None,
            text_content: None,
            children: Vec::new(),
            parent: parent,
        }
    }
}

impl<'html> Tree<'html> {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            root: 0,
        }
    }

    pub fn push(&'html mut self, parent: usize, element: &mut Element<'html>) -> usize {
        self.list.push(Node::new(parent, element.get()));

        let last_index = self.list.len() - 1;
        self.list[parent].children.push(last_index);

        *element = Element::Ref(&self.list[last_index].value);

        return last_index;
    }
}
