use super::cursor::CursorOps;
use super::cursor::{Cursor, ScopedCursor};
use super::multiplexer::{DocumentPosition, SaveHit};
use crate::store::Store;
use crate::{QuerySpec, SelectionKind, XHtmlElement, dbg_print};

/*
 * A Selection works runs the fsm's using 2 types of tasks:
 * 1) the cursor tasks; this is a task that starts in the begining and always picks the last path.
 * 2) the scoped tasks; this is a task that is triggered by the cursor task of an other scoped task.
 *  The important distinction is that the scoped task terminates at a set scope depth (when <= to current depth: terminate).
 */

type ScopedCursorVec = Vec<ScopedCursor>;

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
        tree: &Q,
        store: &mut Store<'html, 'query>,
        element: XHtmlElement<'html>,
        fsm: &mut impl CursorOps<'query, 'html>,
    ) -> SaveHit {
        // I can't check for this anymore, since the save is not instant and the fsm position is moved afterwards
        //debug_assert!(fsm.is_save_point(tree));

        let section = tree.get_selection(fsm.get_position().selection);

        let element_pointer = store.push(fsm.get_parent(), section, element);
        if !tree.is_last_save_point(fsm.get_position()) {
            fsm.set_parent(element_pointer);
        }

        SaveHit {
            element_id: element_pointer,
            save_inner_html: section.save.inner_html,
            save_text_content: section.save.text_content,
        }
    }

    pub fn next(
        &mut self,
        element: &XHtmlElement<'html>,
        document_position: &DocumentPosition,
        store: &mut Store<'html, 'query>,
        save_hits: &mut Vec<SaveHit>,
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
                save_hits.push(Self::save_element(
                    self.query,
                    store,
                    element.clone(),
                    &mut new_scoped_fsm,
                ));

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
                save_hits.push(Self::save_element(self.query, store, element.clone(), fsm));

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

    pub fn back(&mut self, element: &'html str, document_position: &DocumentPosition) -> bool {
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
    use crate::{
        Element, ElementId, Position, Query, QuerySectionId, Reader, Save, TransitionId,
        XHtmlElement,
    };
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
            &mut Vec::new(),
        );

        assert!(store.get("div a").is_none());

        assert_eq!(
            selection.fsm,
            Cursor {
                parent: NULL_PARENT,
                position: Position {
                    selection: QuerySectionId(0),
                    state: TransitionId(1),
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
                    selection: QuerySectionId(0),
                    state: TransitionId(0),
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
            &mut Vec::new(),
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
                        selection: QuerySectionId(0),
                        state: TransitionId(0),
                    },
                },
                ScopedCursor {
                    scope_depth: 1,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: QuerySectionId(0),
                        state: TransitionId(1),
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
            &mut Vec::new(),
        );

        assert!(store.get("div p.class").is_none());

        assert_eq!(
            selection.fsm,
            Cursor {
                parent: NULL_PARENT,
                position: Position {
                    selection: QuerySectionId(0),
                    state: TransitionId(1),
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
                    selection: QuerySectionId(0),
                    state: TransitionId(0),
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
            &mut Vec::new(),
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
                    selection: QuerySectionId(2),
                    state: TransitionId(3),
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
                        selection: QuerySectionId(0),
                        state: TransitionId(0),
                    },
                },
                // ` p.class`
                ScopedCursor {
                    scope_depth: 1,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: QuerySectionId(0),
                        state: TransitionId(1),
                    },
                },
                // `> span`
                ScopedCursor {
                    scope_depth: 1,
                    parent: ElementId(0),
                    position: Position {
                        selection: QuerySectionId(1),
                        state: TransitionId(2),
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
            &mut Vec::new(),
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
                    selection: QuerySectionId(0),
                    state: TransitionId(0),
                },
                match_stack: smallvec![0],
                end: true,
            }
        );

        store.text_content.push(&reader, 4);
        let _ = selection.back(
            "div",
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
        );

        assert!(selection.scoped_fsms.is_empty());

        assert_eq!(
            selection.fsm,
            Cursor {
                parent: NULL_PARENT,
                position: Position {
                    selection: QuerySectionId(0),
                    state: TransitionId(0),
                },
                match_stack: smallvec![],
                end: false,
            }
        );
    }
}
