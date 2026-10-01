const NESTING_LIMIT_MESSAGE: &str = "selector nesting exceeds the maximum depth";

/// Why a selector was rejected.
///
/// Forgiving selector lists (`:is()`, `:where()`) may only discard
/// alternatives that are invalid CSS. Valid selectors that scah cannot
/// evaluate must reject the whole selector, otherwise the query would
/// silently return fewer elements than a browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorParseErrorKind {
    Invalid,
    Unsupported,
    NestingLimit,
}

#[derive(Debug, Clone)]
pub struct SelectorParseError {
    message: &'static str,
    position: usize,
    kind: SelectorParseErrorKind,
}

impl PartialEq for SelectorParseError {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message && self.position == other.position
    }
}

impl Eq for SelectorParseError {}

impl SelectorParseError {
    /// The selector is not valid CSS.
    pub(crate) fn new(message: &'static str, position: usize) -> Self {
        Self {
            message,
            position,
            kind: SelectorParseErrorKind::Invalid,
        }
    }

    /// The selector may be valid CSS, but scah cannot evaluate it.
    pub(crate) fn unsupported(message: &'static str, position: usize) -> Self {
        Self {
            message,
            position,
            kind: SelectorParseErrorKind::Unsupported,
        }
    }

    pub(crate) fn nesting_limit(position: usize) -> Self {
        Self {
            message: NESTING_LIMIT_MESSAGE,
            position,
            kind: SelectorParseErrorKind::NestingLimit,
        }
    }

    /// Returns true when a forgiving selector list must propagate the error
    /// instead of discarding the alternative that produced it.
    pub(crate) fn is_fatal(&self) -> bool {
        self.kind != SelectorParseErrorKind::Invalid
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
