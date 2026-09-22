const NESTING_LIMIT_MESSAGE: &str = "selector nesting exceeds the maximum depth";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorParseError {
    message: &'static str,
    position: usize,
}

impl SelectorParseError {
    pub(crate) fn new(message: &'static str, position: usize) -> Self {
        Self { message, position }
    }

    /// Nesting-budget errors must not be discarded by forgiving selector lists
    /// (`:is()`, `:where()`).
    pub(crate) fn nesting_limit(position: usize) -> Self {
        Self::new(NESTING_LIMIT_MESSAGE, position)
    }

    pub(crate) fn is_fatal(&self) -> bool {
        self.message == NESTING_LIMIT_MESSAGE
    }

    pub fn message(&self) -> &'static str {
        self.message
    }

    pub fn position(&self) -> usize {
        self.position
    }
}

impl std::fmt::Display for SelectorParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.position)
    }
}

impl std::error::Error for SelectorParseError {}
