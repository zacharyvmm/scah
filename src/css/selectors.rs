use crate::css::element;
use crate::utils::reader::Reader;
use crate::xhtml::element::element::XHtmlElement;

use super::fsm::Selection;
use super::selection_map::{BodyContent, SelectionMap};

// handles checking all the selectors (new and pending)
// handles storing the values
// handles parsing the selector strings on constructor creation

#[derive(Debug, Clone)]
pub enum SelectorQueryKind {
    First,
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementContent {
    // Do I need this ??? I can I not just return all the attributes of the element
    //pub attributes: Vec<&'a str>,
    pub text_content: bool,
    pub inner_html: bool,
}

#[derive(Debug, Clone)]
pub struct SelectorQuery<'a> {
    pub kind: SelectorQueryKind,
    pub query: &'a str,
    pub data: ElementContent,
}

#[derive(Debug, Clone)]
pub struct RequestedContent<'a> {
    index: usize,
    selection: Selection<'a>,
    query: SelectorQuery<'a>,
}

#[derive(Debug, PartialEq)]
pub struct Body<'a> {
    pub idxs: Vec<usize>,
    pub content: BodyContent<'a>,
}

#[derive(Debug)]
pub struct Selectors<'query, 'html> {
    pub map: SelectionMap<'query, 'html>,
    pub selections: Vec<RequestedContent<'query>>,
    pub pending_selectors: Vec<RequestedContent<'query>>,
}

impl<'query, 'html> Selectors<'query, 'html> {
    pub fn new(queries: Vec<SelectorQuery<'query>>) -> Self {
        // How should mapping work in a efficient way
        // IDEA: Index based system, the index is given by order
        let mut selectors = Self {
            map: SelectionMap::new(&queries),
            selections: Vec::new(),
            pending_selectors: Vec::new(),
        };

        for i in 0..queries.len() {
            let mut reader: Reader<'query> = Reader::new(queries[i].query);
            let selection: Selection<'query> = Selection::from(&mut reader);
            selectors.selections.push(RequestedContent {
                index: i,
                selection: selection,
                query: queries[i].clone(),
            });
        }
        println!("{:?}", selectors.selections);

        return selectors;
    }

    pub fn feed(
        &mut self,
        xhtml_element: XHtmlElement<'html>,
        depth: u8,
    ) -> Option<(ElementContent, Body<'html>)> {
        let mut idxs: Vec<usize> = Vec::new();

        let mut text_content: bool = false;
        let mut inner_html: bool = false;

        self.pending_selectors.retain_mut(|req| {
            if req.selection.next(&xhtml_element, depth) {
                if req.selection.done() {
                    // add to map or get innerHtml/textContent
                    if req.query.data.text_content || req.query.data.inner_html {
                        text_content |= req.query.data.text_content;
                        inner_html |= req.query.data.inner_html;
                    }

                    println!("index: {}", req.index);
                    idxs.push(req.index);

                    // remove from list
                    return false;
                }
            }
            return true;
        });

        self.selections.retain_mut(|req| {
            if req.selection.next(&xhtml_element, depth) {
                // add to pending
                if req.selection.done() {
                    if req.query.data.text_content | req.query.data.inner_html {
                        text_content |= req.query.data.text_content;
                        inner_html |= req.query.data.inner_html;
                    }

                    idxs.push(req.index);
                } else {
                    self.pending_selectors.push(req.clone());
                }

                match req.query.kind {
                    SelectorQueryKind::First => {
                        // remove the fsm from current list
                        return false;
                    }
                    SelectorQueryKind::All => {
                        // reset to default values
                        req.selection.reset();
                        return true;
                    }
                }

                // Could be a single element selection
                // fsm.done()
            }

            return true;
        });

        if idxs.len() > 0 {
            if text_content | inner_html {
                return Some((
                    ElementContent {
                        text_content,
                        inner_html,
                    },
                    Body {
                        idxs: idxs,
                        content: BodyContent {
                            element: xhtml_element,
                            text_content: None,
                            inner_html: None,
                        },
                    },
                ));
            }

            let element_index = self.map.add_element(BodyContent {
                element: xhtml_element,
                text_content: None,
                inner_html: None,
            });

            println!("idxs: {:?}", idxs);
            for idx in idxs {
                println!("idx: {:?}", idx);
                self.map.push(idx, element_index);
            }

            return None;
        }

        return None;
    }

    pub fn on_stack_pop(&mut self, body: Body<'html>) {
        let element_index = self.map.add_element(body.content);

        println!("idxs: {:?}", body.idxs);
        for idx in body.idxs {
            println!("idx: {:?}", idx);
            self.map.push(idx, element_index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_() {
        let queries = Vec::from([SelectorQuery {
            kind: SelectorQueryKind::All,
            query: "main.red-background > section#id > a[href]",
            data: ElementContent {
                inner_html: false,
                text_content: false,
                //attributes: Vec::from(["href"]),
            },
        }]);

        let selectors = Selectors::new(queries);
    }
}
