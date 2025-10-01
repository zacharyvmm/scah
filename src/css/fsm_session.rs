use super::fsm::Selection;
use super::parser::pattern_section::PatternSection;
use crate::{css::parser::element, utils::tree::Tree};

use crate::XHtmlElement;

/*enum ElementItem<'html> {
    Element(XHtmlElement<'html>),
    Index(usize),
}

pub struct Element<'html> {
    list: &'html mut Vec<XHtmlElement<'html>>,
    item: ElementItem<'html>,
}

impl<'html> Element<'html> {
    pub fn new(list: &'html mut Vec<XHtmlElement<'html>>, xhtml_element: XHtmlElement<'html>) -> Self {
        Self { list: list, item: ElementItem::Element(xhtml_element)}
    }

    fn get(&self) -> &'_ XHtmlElement<'html>{
        match self.item {
            ElementItem::Element(ref element) => element,
            ElementItem::Index(index) => {
                &self.list[index]
            },
        }
    }

    fn save(&mut self) -> () {
        // 1. replace enum with `index`
        // 2. add element to list

        if matches!(self.item, ElementItem::Index(..)) {
            return;
        }

        let xhtml_element = std::mem::replace(
            &mut self.item,
            ElementItem::Index(self.list.len()) // no need to ajust index since +1-1 = 0
        );

        match xhtml_element {
            ElementItem::Element(element) => {
                self.list.push(element);
            },
            //ElementItem::Index(_) => panic!("Already Saved"),
            ElementItem::Index(_) => unreachable!(),
        }
    }
}*/

pub enum Element<'html> {
    Element(XHtmlElement<'html>),
    Ref(&'html XHtmlElement<'html>),
    None,
}

impl<'html> Element<'html> {
    pub fn get_ref(&self) -> &'_ XHtmlElement<'html> {
        match self {
            Self::Element(element) => &element,
            Self::Ref(element_ref) => element_ref,
            Self::None => panic!("Theirs nothing to get"),
        }
    }

    pub fn get(&mut self) -> XHtmlElement<'html> {
        match self {
            Self::Element(_) => {
                let old_self = std::mem::replace(self, Self::None);
                if let Self::Element(element) = old_self {
                    return element;
                } else {
                    unreachable!();
                }
            }
            Self::Ref(element_ref) => element_ref.clone(),
            Self::None => panic!("Theirs nothing to get"),
        }
    }
}

/*
 * Fsm Session should have the following abilities:
 * 1) try step foward at fsm cursor
 * 2) retry fsm pattern segments beginning (previous to the current cursor) -> branch the tree and make new fsm
 * 3) when fsm completed it has to backtrack when going back to add the InnerHtml and TextContent
 * 4) Send the agrogated information back to the caller
 */

pub struct FsmSession<'query, 'html> {
    list: Vec<PatternSection<'query>>,
    tree: Tree<'html>,

    fsms: Vec<(usize, Selection)>,
}

impl<'query, 'html> FsmSession<'query, 'html> {
    fn new(pattern: Vec<PatternSection<'query>>) -> Self {
        Self {
            list: pattern,
            tree: Tree::new(),

            fsms: Vec::new(),
        }
    }

    fn create_fsm() -> Selection {
        Selection::new()
    }

    pub fn step_foward(&mut self, depth: usize, element: &'html mut Element<'html>) {
        let tree_ptr: *mut Tree = &mut self.tree;

        for (previous_node_idx, fsm) in self.fsms.iter_mut() {
            let next = fsm.next(&self.list, element.get_ref(), depth);

            if next {
                //*previous_node_idx = self.tree.push(*previous_node_idx, element);
                unsafe {
                    *previous_node_idx = (*tree_ptr).push(*previous_node_idx, element);
                }
            }
        }
    }

    pub fn step_back(&self, depth: usize) {}

    fn retry_previous_patterns() {}

    fn bubble_up_content_fill_in() {}
}
