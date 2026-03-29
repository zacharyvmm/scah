use std::fmt::Debug;

use super::cursor::CursorOps;
use super::cursor::{Cursor, ScopedCursor};
use super::multiplexer::DocumentPosition;
use crate::store::ElementId;
use crate::store::Store;
use crate::{QuerySpec, Reader, Save, SelectionKind, XHtmlElement, dbg_print};

type StartIdx = Option<usize>;

#[derive(Debug)]
pub(crate) struct DeferredSave {
    element: ElementId,
    on_depth: super::DepthSize,
    inner_html: StartIdx,
    text_content: StartIdx,
}

/*
 * A Selection works runs the fsm's using 2 types of tasks:
 * 1) the cursor tasks; this is a task that starts in the begining and always picks the last path.
 * 2) the scoped tasks; this is a task that is triggered by the cursor task of an other scoped task.
 *  The important distinction is that the scoped task terminates at a set scope depth (when <= to current depth: terminate).
 */

type ScopedCursorVec = Vec<ScopedCursor>;
type EndTagEventVec = Vec<DeferredSave>;

/// The `QueryExecutor` is an NFA execution engine optimized for streaming StAX events.
///
/// Because CSS selectors like descendant (` `) are non-deterministic (a match can
/// occur at the current depth or any arbitrary depth below it), a single cursor
/// isn't enough.
///
/// ## Execution Model
/// 1. **Fictitious States**: Cursors track their position simply as an index into
///    an array of `Transition`s.
/// 2. **Forking (NFA Threads)**: When a transition allows ambiguity (like a descendant
///    search matching but also allowing subsequent sibling/descendant matches), the
///    engine forks a new `ScopedCursor`. This acts as an independent execution thread
///    exploring that specific branch of the NFA.
/// 3. **Pruning**: `ScopedCursor`s have a `scope_depth`. When the StAX parser emits
///    a close tag that drops the document depth below the cursor's scope, that NFA
///    thread is killed.
pub struct QueryExecutor<'a, Q> {
    pub(crate) query: &'a Q,
    pub(crate) fsm: Cursor,
    pub(crate) scoped_fsms: ScopedCursorVec,
    pub(crate) on_close_tag_events: EndTagEventVec,
}

impl<'a, 'html, 'query: 'html, Q> QueryExecutor<'a, Q>
where
    Q: QuerySpec<'query>,
{
    pub fn new(query: &'a Q) -> Self {
        Self {
            query,
            fsm: Cursor::new(),
            scoped_fsms: Vec::new(),
            on_close_tag_events: Vec::new(),
        }
    }

    fn next_position(
        tree: &Q,
        list: &mut ScopedCursorVec,
        depth: super::DepthSize,
        fsm: &mut impl CursorOps<'query, 'html>,
    ) {
        // 1) child, then 2) sibling, then 2) leaf of tree
        fsm.add_depth(depth);
        if let Some(next_transition) = fsm.get_position().next_transition(tree) {
            fsm.set_state(next_transition);
            fsm.set_end(false);
        } else if let Some(child) = fsm.get_position().next_child(tree) {
            fsm.set_position(child);
            fsm.set_end(false);

            let mut has_sibling = fsm.get_position().next_sibling(tree);
            while let Some(sibling) = has_sibling {
                list.push(ScopedCursor::new(
                    depth,
                    fsm.get_parent(),
                    *fsm.get_position(),
                ));
                dbg_print!("Created Scoped FSM {:#?}", list.last().unwrap());

                fsm.set_position(sibling);
                has_sibling = sibling.next_sibling(tree);
            }
        } else {
            fsm.set_end(true);
        }
    }

    pub fn save_element(
        on_close_tag_events: &mut EndTagEventVec,
        tree: &Q,
        store: &mut Store<'html, 'query>,
        element: XHtmlElement<'html>,
        &DocumentPosition {
            element_depth,
            reader_position,
            text_content_position,
        }: &DocumentPosition,
        fsm: &mut impl CursorOps<'query, 'html>,
    ) {
        // I can't check for this anymore, since the save is not instant and the fsm position is moved afterwards
        //debug_assert!(fsm.is_save_point(tree));

        let section = tree.get_selection(fsm.get_position().selection);

        let element_pointer = store.push(fsm.get_parent(), section, element);
        if !tree.is_last_save_point(fsm.get_position()) {
            fsm.set_parent(element_pointer);
        }

        let Save {
            inner_html,
            text_content,
        } = &section.save;

        on_close_tag_events.push(DeferredSave {
            element: element_pointer,
            on_depth: element_depth,
            inner_html: if *inner_html {
                // Since thiis is triggered on opening tag, the start is the current position in the content
                // array is about the previous elements text content item, thus I need to add 1 to get the correct position
                // Their could be a BUG here if there is no text content ("" -> no item added)
                Some(reader_position)
            } else {
                None
            },
            text_content: if *text_content {
                Some(text_content_position)
            } else {
                None
            },
        });
    }

    pub fn next(
        &mut self,
        element: &XHtmlElement<'html>,
        document_position: &DocumentPosition,
        store: &mut Store<'html, 'query>,
    ) {
        for i in 0..self.scoped_fsms.len() {
            if !self.scoped_fsms[i].next(self.query, document_position.element_depth, element) {
                continue;
            }

            dbg_print!("Scoped FSM ({i}) Match with `{:?}`", element);

            if self
                .query
                .is_descendant(self.scoped_fsms[i].get_position().state)
            {
                // This should only be done if the task is not done (meaning it will move forward)
                self.scoped_fsms.push(ScopedCursor::new(
                    document_position.element_depth,
                    self.scoped_fsms[i].parent,
                    self.scoped_fsms[i].position,
                ));
            }

            let mut new_scoped_fsm = self.scoped_fsms[i].clone();

            if self.query.is_save_point(&new_scoped_fsm.position) {
                Self::save_element(
                    &mut self.on_close_tag_events,
                    self.query,
                    store,
                    element.clone(),
                    document_position,
                    &mut new_scoped_fsm,
                );

                dbg_print!("Scoped FSM ({i}) Saved `{:?}`", element);
            }

            if !element.is_self_closing() {
                Self::next_position(
                    self.query,
                    &mut self.scoped_fsms,
                    document_position.element_depth,
                    &mut new_scoped_fsm,
                );
            }

            self.scoped_fsms.push(new_scoped_fsm);

            dbg_print!(">> Scoped FSM's: {:#?}", self.scoped_fsms)
        }

        // STEP 2: check tasks
        let fsm = &mut self.fsm;

        if fsm.next(self.query, document_position.element_depth, element) {
            dbg_print!("FSM Match with `{:?}`", element);

            let is_descendant_combinator = self.query.is_descendant(fsm.position.state);
            let last_save_point = self.query.is_last_save_point(&fsm.position);
            let section_kind = self
                .query
                .get_section_selection_kind(fsm.position.selection);
            let is_all = matches!(section_kind, SelectionKind::All);

            if is_descendant_combinator && (!last_save_point || is_all) {
                self.scoped_fsms.push(ScopedCursor::new(
                    document_position.element_depth,
                    fsm.parent,
                    fsm.position,
                ));
                dbg_print!("Created Scoped FSM {:#?}", self.scoped_fsms.last().unwrap());
            }

            if self.query.is_save_point(&fsm.position) {
                Self::save_element(
                    &mut self.on_close_tag_events,
                    self.query,
                    store,
                    element.clone(),
                    document_position,
                    fsm,
                );

                dbg_print!("FSM Saved `{:?}`", element);
            }

            if !element.is_self_closing() {
                Self::next_position(
                    self.query,
                    &mut self.scoped_fsms,
                    document_position.element_depth,
                    fsm,
                );
                dbg_print!("New FSM {:#?}", fsm);
            }
            dbg_print!("Scoped FSM's: {:#?}", self.scoped_fsms)
        }
    }

    pub fn early_exit(&self) -> bool {
        if let Some(early_exit_section) = self.query.exit_at_section_end() {
            return early_exit_section == self.fsm.position.selection;
        }

        false
    }

    pub fn back(
        &mut self,
        store: &mut Store<'html, 'query>,
        element: &'html str,
        document_position: &DocumentPosition,
        reader: &Reader<'html>,
    ) -> bool {
        //println!("&BACK: {:#?}", self);
        for i in (0..self.on_close_tag_events.len()).rev() {
            let content_trigger = &self.on_close_tag_events[i];
            if content_trigger.on_depth == document_position.element_depth {
                // println!("Closing tag save content for `{element}`");
                let inner_html = {
                    if let Some(start_idx) = content_trigger.inner_html {
                        let slice = reader.slice(start_idx..document_position.reader_position);
                        Some(slice)
                    } else {
                        None
                    }
                };
                let text_content = {
                    if let Some(start_idx) = content_trigger.text_content {
                        if start_idx == usize::MAX {
                            if store.text_content.is_empty() {
                                None
                            } else {
                                Some(0..store.text_content.get_position())
                            }
                        } else if start_idx == store.text_content.get_position() {
                            // No new text content was added after the element opened
                            None
                        } else {
                            // to skip the text content before the element (When the start was just opened, thus thier was no text content yet)
                            Some((start_idx + 1)..store.text_content.get_position())
                        }
                    } else {
                        None
                    }
                };
                store.set_content(content_trigger.element, inner_html, text_content);
                self.on_close_tag_events.remove(i);
            }
        }

        let mut remove_last_x_fsms = 0;
        for scoped_fsm in self.scoped_fsms.iter().rev() {
            if scoped_fsm.scope_depth < document_position.element_depth {
                break;
            }
            self.fsm.parent = scoped_fsm.parent;
            dbg_print!("Removed Scoped FSM ({:#?})", scoped_fsm);
            remove_last_x_fsms += 1;
        }
        self.scoped_fsms
            .truncate(self.scoped_fsms.len() - remove_last_x_fsms);

        let fsm = &mut self.fsm;
        if fsm.back(self.query, document_position.element_depth, element) {
            if fsm.end {
                fsm.end = false;
                fsm.match_stack.pop();
                return true;
            }
            dbg_print!("FSM Before back: {:#?}", fsm);
            fsm.step_backward(self.query);
            dbg_print!("FSM out of `{}`", element);
            dbg_print!("FSM After back: {:#?}", fsm);
            return true;
        }

        false
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::{Element, Position, Query, Reader, Save, XHtmlElement};
    use smallvec::smallvec;

    const NULL_PARENT: ElementId = ElementId(usize::MAX);

    #[test]
    fn test_fsm_next_descendant() {
        let query = &Query::all("div a", Save::none()).unwrap().build();

        let mut store = Store::default();

        let mut selection = QueryExecutor::new(query);

        selection.next(
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: &[],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &mut store,
        );

        assert!(store.get("div a").is_none());

        assert_eq!(
            selection.fsm,
            Cursor {
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 1
                },
                match_stack: smallvec![0],
                end: false,
            }
        );

        assert_eq!(
            selection.scoped_fsms.to_vec(),
            vec![ScopedCursor {
                scope_depth: 0,
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 0
                },
            }]
        );

        selection.next(
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: &[],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 1,
            },
            &mut store,
        );

        assert_eq!(store.get("div a").unwrap().count(), 1);
        let children = store.get("div a").unwrap();

        let children: Vec<&Element> = children.collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "a");

        assert_eq!(
            selection.scoped_fsms.to_vec(),
            vec![
                ScopedCursor {
                    scope_depth: 0,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: 0,
                        state: 0
                    },
                },
                ScopedCursor {
                    scope_depth: 1,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: 0,
                        state: 1
                    },
                }
            ]
        );
    }

    #[test]
    fn test_complex_fsm_query() {
        let query = &Query::first("div p.class", Save::none())
            .unwrap()
            .then(|p| Ok([p.first("span", Save::none())?, p.first("a", Save::none())?]))
            .unwrap()
            .build();

        let mut store = Store::default();
        let mut selection = QueryExecutor::new(query);

        selection.next(
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: &[],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &mut store,
        );

        assert!(store.get("div p.class").is_none());

        assert_eq!(
            selection.fsm,
            Cursor {
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 1
                },
                match_stack: smallvec![0],
                end: false,
            }
        );

        assert_eq!(selection.scoped_fsms.len(), 1);
        assert_eq!(
            selection.scoped_fsms[0],
            ScopedCursor {
                scope_depth: 0,
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 0
                },
            }
        );

        selection.next(
            &XHtmlElement {
                name: "p",
                id: None,
                class: Some("class"),
                attributes: &[],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 1,
            },
            &mut store,
        );

        assert_eq!(store.get("div p.class").unwrap().count(), 1);
        let children = store.get("div p.class").unwrap();
        let children: Vec<&Element> = children.collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "p");
        assert_eq!(children[0].class, Some("class"));

        assert_eq!(
            selection.fsm,
            Cursor {
                parent: ElementId(0),
                position: Position {
                    selection: 2,
                    state: 3
                },
                match_stack: smallvec![0, 1],
                end: false,
            }
        );

        assert_eq!(
            selection.scoped_fsms.to_vec(),
            vec![
                // ` div`
                ScopedCursor {
                    scope_depth: 0,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: 0,
                        state: 0
                    },
                },
                // ` p.class`
                ScopedCursor {
                    scope_depth: 1,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: 0,
                        state: 1
                    },
                },
                // `> span`
                ScopedCursor {
                    scope_depth: 1,
                    parent: ElementId(0),
                    position: Position {
                        selection: 1,
                        state: 2
                    },
                },
            ]
        );
    }

    #[test]
    fn test_simple_open_close() {
        let query = Query::first("div", Save::none()).unwrap().build();

        let mut store = Store::default();
        let mut selection = QueryExecutor::new(&query);

        let reader = Reader::new("<div></div>");

        selection.next(
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: &[],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &mut store,
        );
        store.text_content.set_start(4);
        println!("{:?}", store);
        println!("{:?}", selection.fsm);

        assert!(selection.scoped_fsms.is_empty());

        assert_eq!(
            selection.fsm,
            Cursor {
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 0
                },
                match_stack: smallvec![0],
                end: true,
            }
        );

        store.text_content.push(&reader, 4);
        let _ = selection.back(
            &mut store,
            "div",
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &reader,
        );

        assert!(selection.scoped_fsms.is_empty());

        assert_eq!(
            selection.fsm,
            Cursor {
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 0
                },
                match_stack: smallvec![],
                end: false,
            }
        );
    }
}
