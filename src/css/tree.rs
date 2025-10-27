use crate::XHtmlElement;

// This is essentially a Selection DOM or Selection Object Model
pub struct Tree<'html> {
    list: Vec<Node<'html>>,
    root: usize,
}

#[derive(PartialEq, Debug)]
pub struct Node<'html> {
    pub value: XHtmlElement<'html>, // for the rust lib it's better UX to have a reference instead of a index
    pub inner_html: Option<&'html str>,
    pub text_content: Option<&'html str>, // After concat evaluation

    pub children: Vec<usize>,
}

impl<'html> Node<'html> {
    fn new(element: XHtmlElement<'html>) -> Self {
        Self {
            value: element,
            inner_html: None,
            text_content: None,
            children: Vec::new(),
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

    pub fn push(&mut self, parent: usize, element: XHtmlElement<'html>) -> usize {
        let last_index = self.list.len();
        self.list.push(Node::new(element));
        if self.list.len() != 1 {
            self.list[parent].children.push(last_index);
        }
        return last_index;
    }

    pub fn set_content(&mut self, position:usize, inner_html: Option<&'html str>, text_content: Option<&'html str>) {
        self.list[position].inner_html = inner_html;
        self.list[position].text_content = text_content;
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::reader::Reader;

    use super::*;

    #[test]
    fn test_build_tree() {
        /*
         * a.class --> span#id --> p
         *         \-> p
         */
        let mut tree = Tree::new();

        // SETUP Elements
        let first = XHtmlElement::from(&mut Reader::new("a class=\"class\""));
        let second = XHtmlElement::from(&mut Reader::new("span id=\"id\""));
        let second_extended = XHtmlElement::from(&mut Reader::new("p"));
        let second_alternate = XHtmlElement::from(&mut Reader::new("p"));

        let mut index = tree.push(0, first);
        _ = tree.push(index, second_alternate);
        index = tree.push(index, second);
        _ = tree.push(index, second_extended);

        assert_eq!(
            tree.list,
            vec![
                Node {
                    value: XHtmlElement {
                        name: "a",
                        class: Some("class"),
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![1, 2],
                    inner_html: None,
                    text_content: None,
                },
                Node {
                    value: XHtmlElement {
                        name: "p",
                        class: None,
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![],
                    inner_html: None,
                    text_content: None,
                },
                Node {
                    value: XHtmlElement {
                        name: "span",
                        class: None,
                        id: Some("id"),
                        attributes: Vec::new()
                    },
                    children: vec![3],
                    inner_html: None,
                    text_content: None,
                },
                Node {
                    value: XHtmlElement {
                        name: "p",
                        class: None,
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![],
                    inner_html: None,
                    text_content: None,
                },
            ]
        )
    }
}
