use crate::utils::reader::Reader;
use crate::xhtml::element::element::XHtmlElement;

use super::selection_map::SelectionMap;
use super::fsm::Selection;

// handles checking all the selectors (new and pending)
// handles storing the values
// handles parsing the selector strings on constructor creation

#[derive(Clone)]
enum SelectorQueryKind {
    First,
    All,
}

struct ElementContent<'a> {
    attributes: Vec<&'a str>,
    text_content: bool,
    inner_html: bool,
}

struct SelectorQuery<'a> {
    kind: SelectorQueryKind,
    query: &'a str,
    data: ElementContent<'a>,
}

struct Selectors<'query, 'html> {
    map: SelectionMap<'query, 'html>,
    selections: Vec<(SelectorQueryKind, Selection<'query>)>,
    pending_selectors: Vec<Selection<'query>>,
}

impl<'query, 'html> Selectors<'query, 'html> {
    pub fn new(queries: Vec<SelectorQuery<'query>>) -> Self {
        Self {
            map: SelectionMap::new(),
            selections: queries.iter().map(|query| {
                let mut reader: Reader<'query> = Reader::new(query.query);
                let selection: Selection<'query> = Selection::from(&mut reader);
                
                return (query.kind.clone(), selection);
            }).collect(),
            pending_selectors: Vec::new(),
        }
    }

    pub fn feed(&mut self, xhtml_element: &XHtmlElement<'html>, depth: u8) {
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
                    },
                    SelectorQueryKind::All => {
                        // reset to default values
                        fsm.reset();
                        return true;
                    },
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
        let queries = Vec::from([
            SelectorQuery {
                kind: SelectorQueryKind::All,
                query: "main.red-background > section#id > a[href]",
                data: ElementContent {
                    inner_html: false,
                    text_content: false,
                    attributes: Vec::from(["href"]),
                } 
            }
        ]);

        let selectors = Selectors::new(queries);
    }
}