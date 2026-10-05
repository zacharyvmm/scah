//! Cached tag properties for the HTML parser and normalized-text extraction.
//!
//! Tag names are resolved to a [`TagId`] once per tag. Parser and
//! normalized-text properties are then a single load from [`CLASSIFIED`], a
//! table indexed by that id.

use scah_query_ir::TagId;

/// Cached, overlapping properties of an HTML tag required by the parser.
///
/// This is a bit set rather than a single tag enum because one tag can have
/// several independent parser behaviors. For example, `table` closes an open
/// paragraph and is also a barrier for multiple scope kinds, while `hr` is
/// both void and paragraph-closing. Keeping those properties together lets us
/// classify a tag name once, store the result on the open-element stack, and
/// answer hot-path membership and scope queries with integer mask tests rather
/// than repeatedly matching or comparing the tag name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TagFlags(u32);

/// Normalized-text semantics for a tag name.
///
/// Only consulted when the parse captures normalized `text`. Kept as a
/// separate bitset so the parser-only classifier can stay lean.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextTagFlags(u16);

/// Combined parser + text classification from a single name lookup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassifiedTag {
    pub parser: TagFlags,
    pub text: TextTagFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Default,
    ListItem,
    Button,
    Table,
    Select,
}

impl TagFlags {
    const VOID: u32 = 1 << 0;
    const CLOSES_P: u32 = 1 << 1;
    const P: u32 = 1 << 2;
    const BUTTON: u32 = 1 << 3;
    const LI: u32 = 1 << 4;
    const DT_DD: u32 = 1 << 5;
    const OPTION: u32 = 1 << 6;
    const OPTGROUP: u32 = 1 << 7;
    const TR: u32 = 1 << 8;
    const CELL: u32 = 1 << 9;
    const TABLE_SCOPE: u32 = 1 << 10;
    const DEFAULT_BARRIER: u32 = 1 << 11;
    const LIST_BARRIER: u32 = 1 << 12;
    const TABLE_BARRIER: u32 = 1 << 13;
    const HTML_TEMPLATE: u32 = 1 << 14;
    const RAW_SCRIPT: u32 = 1 << 15;
    const RAW_STYLE: u32 = 1 << 16;
    const RAW_TEXTAREA: u32 = 1 << 17;
    const RAW_TITLE: u32 = 1 << 18;

    #[inline]
    pub(crate) const fn bits(self) -> u32 {
        self.0
    }

    #[inline]
    pub(crate) const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub(crate) const P_MASK: Self = Self(Self::P);
    pub(crate) const BUTTON_MASK: Self = Self(Self::BUTTON);
    pub(crate) const LI_MASK: Self = Self(Self::LI);
    pub(crate) const DT_DD_MASK: Self = Self(Self::DT_DD);
    pub(crate) const OPTION_MASK: Self = Self(Self::OPTION);
    pub(crate) const OPTGROUP_MASK: Self = Self(Self::OPTGROUP);
    pub(crate) const TR_MASK: Self = Self(Self::TR);
    pub(crate) const CELL_MASK: Self = Self(Self::CELL);

    /// Parser properties of `name`.
    #[inline]
    pub fn classify(name: &str) -> Self {
        Self::of(TagId::of(name))
    }

    /// Parser properties of `tag`.
    #[inline(always)]
    pub fn of(tag: TagId) -> Self {
        CLASSIFIED[tag.index()].parser
    }

    const fn compute(tag: TagId) -> Self {
        let flags = match tag {
            TagId::AREA
            | TagId::BASE
            | TagId::BR
            | TagId::COL
            | TagId::EMBED
            | TagId::IMG
            | TagId::INPUT
            | TagId::LINK
            | TagId::META
            | TagId::PARAM
            | TagId::SOURCE
            | TagId::TRACK
            | TagId::WBR => Self::VOID,
            TagId::HR => Self::VOID | Self::CLOSES_P,
            TagId::ADDRESS
            | TagId::ARTICLE
            | TagId::ASIDE
            | TagId::BLOCKQUOTE
            | TagId::DIV
            | TagId::DL
            | TagId::FIELDSET
            | TagId::FOOTER
            | TagId::FORM
            | TagId::H1
            | TagId::H2
            | TagId::H3
            | TagId::H4
            | TagId::H5
            | TagId::H6
            | TagId::HEADER
            | TagId::MAIN
            | TagId::NAV
            | TagId::PRE
            | TagId::SECTION => Self::CLOSES_P,
            TagId::P => Self::CLOSES_P | Self::P,
            TagId::OL | TagId::UL => Self::CLOSES_P | Self::LIST_BARRIER,
            TagId::TABLE => Self::CLOSES_P | Self::DEFAULT_BARRIER | Self::TABLE_BARRIER,
            TagId::BUTTON => Self::BUTTON,
            TagId::LI => Self::LI,
            TagId::DT | TagId::DD => Self::DT_DD,
            TagId::OPTION => Self::OPTION,
            TagId::OPTGROUP => Self::OPTGROUP,
            TagId::TR => Self::TR | Self::TABLE_SCOPE,
            TagId::TD | TagId::TH => Self::CELL | Self::DEFAULT_BARRIER,
            TagId::THEAD | TagId::TBODY | TagId::TFOOT | TagId::CAPTION | TagId::COLGROUP => {
                Self::TABLE_SCOPE
            }
            TagId::APPLET | TagId::MARQUEE | TagId::OBJECT => Self::DEFAULT_BARRIER,
            TagId::HTML | TagId::TEMPLATE => Self::HTML_TEMPLATE | Self::TABLE_BARRIER,
            TagId::SCRIPT => Self::RAW_SCRIPT,
            TagId::STYLE => Self::RAW_STYLE,
            TagId::TEXTAREA => Self::RAW_TEXTAREA,
            TagId::TITLE => Self::RAW_TITLE,
            _ => 0,
        };
        Self(flags)
    }

    #[inline]
    pub(crate) const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[inline]
    pub const fn is_void(self) -> bool {
        self.0 & Self::VOID != 0
    }

    #[inline]
    pub const fn closes_open_p(self) -> bool {
        self.0 & Self::CLOSES_P != 0
    }

    #[inline]
    pub(crate) const fn can_trigger_implied_close(self) -> bool {
        self.0
            & (Self::CLOSES_P
                | Self::BUTTON
                | Self::LI
                | Self::DT_DD
                | Self::OPTION
                | Self::OPTGROUP
                | Self::TR
                | Self::CELL)
            != 0
    }

    #[inline]
    pub(crate) const fn close_scope(self) -> ScopeKind {
        if self.0 & (Self::LI | Self::DT_DD) != 0 {
            ScopeKind::ListItem
        } else if self.0 & Self::BUTTON != 0 {
            ScopeKind::Button
        } else if self.0 & (Self::TR | Self::CELL | Self::TABLE_SCOPE) != 0 {
            ScopeKind::Table
        } else if self.0 & (Self::OPTION | Self::OPTGROUP) != 0 {
            ScopeKind::Select
        } else {
            ScopeKind::Default
        }
    }

    #[inline]
    pub const fn is_scope_barrier(self, scope: ScopeKind) -> bool {
        if self.0 & Self::HTML_TEMPLATE != 0 {
            return true;
        }

        match scope {
            ScopeKind::Default => self.0 & Self::DEFAULT_BARRIER != 0,
            ScopeKind::ListItem => self.0 & (Self::DEFAULT_BARRIER | Self::LIST_BARRIER) != 0,
            ScopeKind::Button => self.0 & (Self::DEFAULT_BARRIER | Self::BUTTON) != 0,
            ScopeKind::Table => self.0 & Self::TABLE_BARRIER != 0,
            ScopeKind::Select => self.0 & (Self::OPTION | Self::OPTGROUP) == 0,
        }
    }

    #[inline]
    pub(crate) const fn raw_text_close_tag(self) -> Option<&'static str> {
        if self.0 & Self::RAW_SCRIPT != 0 {
            Some("</script")
        } else if self.0 & Self::RAW_STYLE != 0 {
            Some("</style")
        } else if self.0 & Self::RAW_TEXTAREA != 0 {
            Some("</textarea")
        } else if self.0 & Self::RAW_TITLE != 0 {
            Some("</title")
        } else {
            None
        }
    }
}

#[allow(dead_code)]
impl TextTagFlags {
    const BLOCK: u16 = 1 << 0;
    const BREAK: u16 = 1 << 1;
    const ROW: u16 = 1 << 2;
    const CELL: u16 = 1 << 3;
    const SUPPRESSED: u16 = 1 << 4;
    const PREFORMATTED: u16 = 1 << 5;

    /// Normalized-text properties of `name`.
    #[inline]
    pub fn classify(name: &str) -> Self {
        Self::of(TagId::of(name))
    }

    /// Normalized-text properties of `tag`.
    #[inline(always)]
    pub fn of(tag: TagId) -> Self {
        CLASSIFIED[tag.index()].text
    }

    const fn compute(tag: TagId) -> Self {
        let flags = match tag {
            TagId::BR | TagId::HR => Self::BREAK,
            TagId::ADDRESS
            | TagId::ARTICLE
            | TagId::ASIDE
            | TagId::BLOCKQUOTE
            | TagId::DIV
            | TagId::DL
            | TagId::FIELDSET
            | TagId::FOOTER
            | TagId::FORM
            | TagId::HEADER
            | TagId::MAIN
            | TagId::NAV
            | TagId::SECTION
            | TagId::H1
            | TagId::H2
            | TagId::H3
            | TagId::H4
            | TagId::H5
            | TagId::H6
            | TagId::P
            | TagId::OL
            | TagId::UL
            | TagId::TABLE
            | TagId::LI
            | TagId::DT
            | TagId::DD
            | TagId::THEAD
            | TagId::TBODY
            | TagId::TFOOT
            | TagId::COLGROUP
            | TagId::CAPTION
            | TagId::BODY
            | TagId::DETAILS
            | TagId::DIALOG
            | TagId::FIGCAPTION
            | TagId::FIGURE
            | TagId::HGROUP
            | TagId::LEGEND
            | TagId::MENU
            | TagId::SEARCH
            | TagId::SUMMARY => Self::BLOCK,
            TagId::TR => Self::ROW | Self::BLOCK,
            TagId::TD | TagId::TH => Self::CELL,
            TagId::TEMPLATE | TagId::SCRIPT | TagId::STYLE => Self::SUPPRESSED,
            TagId::TEXTAREA => Self::PREFORMATTED,
            TagId::PRE => Self::BLOCK | Self::PREFORMATTED,
            _ => 0,
        };
        Self(flags)
    }

    #[inline]
    pub(crate) const fn is_block(self) -> bool {
        self.0 & Self::BLOCK != 0
    }

    #[inline]
    pub(crate) const fn is_break(self) -> bool {
        self.0 & Self::BREAK != 0
    }

    #[inline]
    pub(crate) const fn is_row(self) -> bool {
        self.0 & Self::ROW != 0
    }

    #[inline]
    pub(crate) const fn is_cell(self) -> bool {
        self.0 & Self::CELL != 0
    }

    #[inline]
    pub(crate) const fn is_suppressed(self) -> bool {
        self.0 & Self::SUPPRESSED != 0
    }

    #[inline]
    pub(crate) const fn is_preformatted(self) -> bool {
        self.0 & Self::PREFORMATTED != 0
    }

    /// Separator queued after this element's content in normalized text.
    #[inline]
    #[allow(dead_code)] // mirrored on TextElementFlags for the open-stack close path
    pub(crate) fn post_text_separator(self) -> Option<super::text_state::PendingSeparator> {
        use super::text_state::PendingSeparator;
        if self.is_break() || self.is_block() || self.is_row() {
            Some(PendingSeparator::LineBreak)
        } else if self.is_cell() {
            Some(PendingSeparator::Tab)
        } else {
            None
        }
    }
}

impl ClassifiedTag {
    /// Parser and normalized-text properties of `name`.
    #[cfg(test)]
    pub fn classify(name: &str) -> Self {
        Self::of(TagId::of(name))
    }

    /// Parser and normalized-text properties of `tag`: one table load.
    #[inline(always)]
    pub fn of(tag: TagId) -> Self {
        CLASSIFIED[tag.index()]
    }
}

/// Properties of every [`TagId`], computed at compile time.
static CLASSIFIED: [ClassifiedTag; TagId::COUNT] = {
    let mut table = [ClassifiedTag {
        parser: TagFlags(0),
        text: TextTagFlags(0),
    }; TagId::COUNT];
    let mut index = 0;
    while index < TagId::COUNT {
        let tag = TagId::from_index(index);
        table[index] = ClassifiedTag {
            parser: TagFlags::compute(tag),
            text: TextTagFlags::compute(tag),
        };
        index += 1;
    }
    table
};

#[cfg(test)]
mod tests {
    use super::{ClassifiedTag, TagFlags, TextTagFlags};

    #[test]
    fn mixed_case_names_have_the_same_classification() {
        for (lowercase, mixed_case) in [
            ("div", "DiV"),
            ("hr", "HR"),
            ("button", "BuTtOn"),
            ("optgroup", "OPTGROUP"),
            ("colgroup", "ColGroup"),
            ("template", "TEMPLATE"),
            ("textarea", "TextArea"),
        ] {
            assert_eq!(
                TagFlags::classify(lowercase),
                TagFlags::classify(mixed_case)
            );
            assert_eq!(
                TextTagFlags::classify(lowercase),
                TextTagFlags::classify(mixed_case)
            );
            assert_eq!(
                ClassifiedTag::classify(lowercase),
                ClassifiedTag::classify(mixed_case)
            );
        }
    }

    #[test]
    fn unknown_names_have_no_flags() {
        assert_eq!(TagFlags::classify("custom-element"), TagFlags::default());
        assert_eq!(TagFlags::classify("CUSTOM-ELEMENT"), TagFlags::default());
        assert_eq!(
            TextTagFlags::classify("custom-element"),
            TextTagFlags::default()
        );
        assert_eq!(
            ClassifiedTag::classify("custom-element"),
            ClassifiedTag::default()
        );
    }

    #[test]
    fn combined_matches_split_classifiers() {
        for name in [
            "div",
            "p",
            "br",
            "hr",
            "pre",
            "td",
            "tr",
            "script",
            "style",
            "template",
            "textarea",
            "body",
            "span",
            "custom-element",
            "TABLE",
        ] {
            let combined = ClassifiedTag::classify(name);
            assert_eq!(
                combined.parser,
                TagFlags::classify(name),
                "parser for {name}"
            );
            assert_eq!(
                combined.text,
                TextTagFlags::classify(name),
                "text for {name}"
            );
        }
    }

    #[test]
    fn text_flags_cover_normalized_semantics() {
        assert!(TextTagFlags::classify("div").is_block());
        assert!(TextTagFlags::classify("br").is_break());
        assert!(TextTagFlags::classify("tr").is_row());
        assert!(TextTagFlags::classify("td").is_cell());
        assert!(TextTagFlags::classify("script").is_suppressed());
        assert!(TextTagFlags::classify("pre").is_preformatted());
        assert!(TextTagFlags::classify("pre").is_block());
        assert!(TextTagFlags::classify("textarea").is_preformatted());
        assert!(!TextTagFlags::classify("span").is_block());
    }

    #[test]
    fn parser_flags_match_historical_main_semantics() {
        assert!(TagFlags::classify("br").is_void());
        assert!(TagFlags::classify("hr").is_void());
        assert!(TagFlags::classify("hr").closes_open_p());
        assert!(TagFlags::classify("pre").closes_open_p());
        assert!(!TagFlags::classify("body").closes_open_p());
        assert!(!TagFlags::classify("body").is_void());
    }

    #[test]
    fn only_implied_close_openers_enter_stack_preparation() {
        for name in ["p", "div", "li", "button", "option", "tr", "td"] {
            assert!(
                TagFlags::classify(name).can_trigger_implied_close(),
                "{name}"
            );
        }
        for name in ["a", "span", "img", "script", "custom-element"] {
            assert!(
                !TagFlags::classify(name).can_trigger_implied_close(),
                "{name}"
            );
        }
    }
}
