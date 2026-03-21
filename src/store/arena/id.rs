pub(crate) use super::iter::Nullable;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct ElementId(pub(crate) usize);

const NULL: usize = usize::MAX;

impl Nullable for ElementId {
    fn is_null(&self) -> bool {
        self.0 == NULL
    }
}
impl Default for ElementId {
    fn default() -> Self {
        Self(NULL)
    }
}

impl From<usize> for ElementId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<ElementId> for usize {
    fn from(value: ElementId) -> Self {
        value.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct QueryId(pub(crate) usize);

impl Nullable for QueryId {
    fn is_null(&self) -> bool {
        self.0 == NULL
    }
}
impl Default for QueryId {
    fn default() -> Self {
        Self(NULL)
    }
}

impl From<usize> for QueryId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<QueryId> for usize {
    fn from(value: QueryId) -> Self {
        value.0
    }
}
