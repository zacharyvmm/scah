use crate::{ElementId, QuerySectionId, TransitionId};

#[derive(Debug, Clone, PartialEq)]
pub struct TraceStore<'html, 'query> {
    events: Vec<TraceEvent<'html, 'query>>,
}

impl<'html, 'query> TraceStore<'html, 'query> {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, event: TraceEvent<'html, 'query>) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[TraceEvent<'html, 'query>] {
        &self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl<'html, 'query> Default for TraceStore<'html, 'query> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraceEvent<'html, 'query> {
    ParseStarted {
        html_len: usize,
        query_count: usize,
    },
    ParseFinished {
        element_count: usize,
        query_node_count: usize,
        attribute_count: usize,
        text_content_len: usize,
    },
    OpenTag {
        tag: &'html str,
        depth: u16,
        reader_position: usize,
        self_closing: bool,
    },
    CloseTag {
        tag: &'html str,
        depth: u16,
        reader_position: usize,
    },
    ImpliedClose {
        tag: &'html str,
        depth: u16,
        reason: ImpliedCloseReason,
    },
    TransitionMatched {
        runner_index: usize,
        cursor: CursorTraceKind,
        selector: &'query str,
        element: &'html str,
        depth: u16,
        selection: QuerySectionId,
        state: TransitionId,
    },
    TransitionRejected {
        runner_index: usize,
        cursor: CursorTraceKind,
        selector: &'query str,
        element: &'html str,
        depth: u16,
        selection: QuerySectionId,
        state: TransitionId,
        reason: TransitionRejectReason,
    },
    ScopedCursorCreated {
        runner_index: usize,
        depth: u16,
        scope_depth: u16,
        parent: ElementId,
        selection: QuerySectionId,
        state: TransitionId,
        reason: ScopedCursorReason,
    },
    ScopedCursorPruned {
        runner_index: usize,
        cursor_index: usize,
        scope_depth: u16,
        close_depth: u16,
        selection: QuerySectionId,
        state: TransitionId,
    },
    ElementSaved {
        runner_index: usize,
        selector: &'query str,
        element: &'html str,
        element_id: ElementId,
        parent_id: ElementId,
        save_inner_html: bool,
        save_text_content: bool,
    },
    ContentFinalized {
        element_id: ElementId,
        tag: &'html str,
        has_inner_html: bool,
        has_text_content: bool,
    },
    EarlyExit {
        runner_index: usize,
        selector: &'query str,
        section: QuerySectionId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorTraceKind {
    Main,
    Scoped { index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopedCursorReason {
    DescendantFork,
    BranchSibling,
    ChildSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpliedCloseReason {
    OpenTagRule,
    MismatchedEndTag,
    EofDrain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionRejectReason {
    DepthGuardFailed,
    PredicateFailed,
}
