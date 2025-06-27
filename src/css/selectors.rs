use crate::utils::reader::Reader;
use crate::xhtml::element::element::XHtmlElement;

use super::fsm::Selection;
use super::selection_map::SelectionMap;

// handles checking all the selectors (new and pending)
// handles storing the values
// handles parsing the selector strings on constructor creation

#[derive(Clone)]
pub enum SelectorQueryKind {
    First,
    All,
}

pub struct ElementContent<'a> {
    pub attributes: Vec<&'a str>,
    pub text_content: bool,
    pub inner_html: bool,
}

pub struct Wait<'a> {
    index: usize,
    content: ElementContent<'a>, // index of the FSM map element
}

pub struct SelectorQuery<'a> {
    pub kind: SelectorQueryKind,
    pub query: &'a str,
    pub data: ElementContent<'a>,
}

pub struct Selectors<'query, 'html> {
    map: SelectionMap<'query, 'html>,
    selections: Vec<(usize, Selection<'query>)>,
    pending_selectors: Vec<(usize, Selection<'query>)>,
}

impl<'query, 'html> Selectors<'query, 'html> {
    pub fn new(queries: Vec<SelectorQuery<'query>>) -> Self {
        // How should mapping work in a efficient way
        // IDEA: Index based system, the index is given by order
        Self {
            map: SelectionMap::new(),
            selections: queries
                .iter()
                .map(|query| {
                    let mut reader: Reader<'query> = Reader::new(query.query);
                    let selection: Selection<'query> = Selection::from(&mut reader);

                    return selection;
                })
                .collect(),
            pending_selectors: Vec::new(),
        }
    }

    pub fn feed(&mut self, xhtml_element: &XHtmlElement<'html>, depth: u8) -> Vec<Wait<'query>> {
        let mut wait_list: Vec<Wait<'query>> = Vec::new();

        self.pending_selectors.retain_mut(|fsm| {
            if fsm.next(xhtml_element, depth) {
                if fsm.done() {
                    // add to map or get innerHtml/textContent

                    // remove from list
                    return false;
                }
            }
            return true;
        });

        self.selections.retain_mut(|(kind, fsm)| {
            if fsm.next(xhtml_element, depth) {
                // add to pending
                self.pending_selectors.push(fsm.clone());
                match kind {
                    SelectorQueryKind::First => {
                        // remove the fsm from current list
                        return false;
                    }
                    SelectorQueryKind::All => {
                        // reset to default values
                        fsm.reset();
                        return true;
                    }
                }

                // Could be a single element selection
                // fsm.done()
            }

            return true;
        });
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
                attributes: Vec::from(["href"]),
            },
        }]);

        let selectors = Selectors::new(queries);
    }
}
