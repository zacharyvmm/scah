//! Dense identifiers for HTML tag names.
//!
//! HTML has a small, fixed vocabulary of tag names, so both the parser and
//! compiled selectors resolve names to a one-byte [`TagId`] once and compare
//! integers afterwards. Names outside the table (custom elements, rare SVG and
//! MathML) share [`TagId::UNKNOWN`] and still need a name comparison.

/// Dense identifier for a known tag name, compared ASCII case-insensitively.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TagId(u8);

/// Longest name a [`Key`] can represent.
const MAX_KEY_LEN: usize = 16;

/// A name's length plus two words that, given the length, determine every
/// byte. Short names load overlapping words instead of looping over bytes.
type Key = (usize, u64, u64);

const UNKNOWN_KEY: Key = (0, 0, 0);

#[inline(always)]
const fn word4(bytes: &[u8], at: usize) -> u64 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as u64
}

#[inline(always)]
const fn word8(bytes: &[u8], at: usize) -> u64 {
    word4(bytes, at) | word4(bytes, at + 4) << 32
}

/// ASCII-lowercases every byte of `word` without branches.
#[inline(always)]
const fn lowercase(word: u64) -> u64 {
    const ONES: u64 = 0x0101_0101_0101_0101;
    let ascii = word & (ONES * 0x7f);
    let at_least_a = ascii + ONES * (0x80 - b'A' as u64);
    let past_z = ascii + ONES * (0x80 - b'Z' as u64 - 1);
    let upper = at_least_a & !past_z & !word & (ONES * 0x80);
    word | (upper >> 2)
}

#[inline(always)]
const fn key(name: &[u8]) -> Key {
    let len = name.len();
    let (low, high) = match len {
        1..=3 => (
            name[0] as u64 | (name[len / 2] as u64) << 8 | (name[len - 1] as u64) << 16,
            0,
        ),
        4..=8 => (word4(name, 0) | word4(name, len - 4) << 32, 0),
        9..=MAX_KEY_LEN => (word8(name, 0), word8(name, len - 8)),
        _ => return UNKNOWN_KEY,
    };
    (len, lowercase(low), lowercase(high))
}

macro_rules! known_tags {
    ($($ident:ident = $name:literal,)*) => {
        #[repr(u8)]
        #[allow(non_camel_case_types, clippy::upper_case_acronyms, dead_code)]
        enum Known {
            Unknown,
            $($ident,)*
        }

        #[allow(dead_code)]
        impl TagId {
            $(pub const $ident: Self = Self(Known::$ident as u8);)*
        }

        /// Number of distinct ids, including [`TagId::UNKNOWN`].
        const TAG_COUNT: usize = 1 + [$($name),*].len();

        const NAMES: [&str; TAG_COUNT] = ["", $($name),*];

        mod keys {
            $(pub(super) const $ident: super::Key = super::key($name.as_bytes());)*
        }

        #[inline(always)]
        const fn lookup(key: Key) -> TagId {
            // Destructured const patterns switch on length first.
            match key {
                $(keys::$ident => TagId::$ident,)*
                _ => TagId::UNKNOWN,
            }
        }
    };
}

// HTML elements, plus the common SVG and MathML roots and SVG shapes.
known_tags! {
    A = "a",
    ABBR = "abbr",
    ADDRESS = "address",
    APPLET = "applet",
    AREA = "area",
    ARTICLE = "article",
    ASIDE = "aside",
    AUDIO = "audio",
    B = "b",
    BASE = "base",
    BDI = "bdi",
    BDO = "bdo",
    BIG = "big",
    BLOCKQUOTE = "blockquote",
    BODY = "body",
    BR = "br",
    BUTTON = "button",
    CANVAS = "canvas",
    CAPTION = "caption",
    CENTER = "center",
    CITE = "cite",
    CODE = "code",
    COL = "col",
    COLGROUP = "colgroup",
    DATA = "data",
    DATALIST = "datalist",
    DD = "dd",
    DEL = "del",
    DETAILS = "details",
    DFN = "dfn",
    DIALOG = "dialog",
    DIV = "div",
    DL = "dl",
    DT = "dt",
    EM = "em",
    EMBED = "embed",
    FIELDSET = "fieldset",
    FIGCAPTION = "figcaption",
    FIGURE = "figure",
    FONT = "font",
    FOOTER = "footer",
    FORM = "form",
    FRAME = "frame",
    FRAMESET = "frameset",
    H1 = "h1",
    H2 = "h2",
    H3 = "h3",
    H4 = "h4",
    H5 = "h5",
    H6 = "h6",
    HEAD = "head",
    HEADER = "header",
    HGROUP = "hgroup",
    HR = "hr",
    HTML = "html",
    I = "i",
    IFRAME = "iframe",
    IMG = "img",
    INPUT = "input",
    INS = "ins",
    KBD = "kbd",
    LABEL = "label",
    LEGEND = "legend",
    LI = "li",
    LINK = "link",
    MAIN = "main",
    MAP = "map",
    MARK = "mark",
    MARQUEE = "marquee",
    MENU = "menu",
    META = "meta",
    METER = "meter",
    NAV = "nav",
    NOBR = "nobr",
    NOEMBED = "noembed",
    NOFRAMES = "noframes",
    NOSCRIPT = "noscript",
    OBJECT = "object",
    OL = "ol",
    OPTGROUP = "optgroup",
    OPTION = "option",
    OUTPUT = "output",
    P = "p",
    PARAM = "param",
    PICTURE = "picture",
    PLAINTEXT = "plaintext",
    PRE = "pre",
    PROGRESS = "progress",
    Q = "q",
    RP = "rp",
    RT = "rt",
    RUBY = "ruby",
    S = "s",
    SAMP = "samp",
    SCRIPT = "script",
    SEARCH = "search",
    SECTION = "section",
    SELECT = "select",
    SLOT = "slot",
    SMALL = "small",
    SOURCE = "source",
    SPAN = "span",
    STRIKE = "strike",
    STRONG = "strong",
    STYLE = "style",
    SUB = "sub",
    SUMMARY = "summary",
    SUP = "sup",
    TABLE = "table",
    TBODY = "tbody",
    TD = "td",
    TEMPLATE = "template",
    TEXTAREA = "textarea",
    TFOOT = "tfoot",
    TH = "th",
    THEAD = "thead",
    TIME = "time",
    TITLE = "title",
    TR = "tr",
    TRACK = "track",
    TT = "tt",
    U = "u",
    UL = "ul",
    VAR = "var",
    VIDEO = "video",
    WBR = "wbr",
    XMP = "xmp",
    MATH = "math",
    SVG = "svg",
    CIRCLE = "circle",
    CLIPPATH = "clippath",
    DEFS = "defs",
    ELLIPSE = "ellipse",
    FOREIGNOBJECT = "foreignobject",
    G = "g",
    IMAGE = "image",
    LINE = "line",
    LINEARGRADIENT = "lineargradient",
    MASK = "mask",
    PATH = "path",
    PATTERN = "pattern",
    POLYGON = "polygon",
    POLYLINE = "polyline",
    RADIALGRADIENT = "radialgradient",
    RECT = "rect",
    STOP = "stop",
    SYMBOL = "symbol",
    TEXT = "text",
    TSPAN = "tspan",
    USE = "use",
}

impl TagId {
    /// Shared id for every name outside the known-tag table.
    pub const UNKNOWN: Self = Self(0);

    /// Number of distinct ids, including [`TagId::UNKNOWN`].
    pub const COUNT: usize = TAG_COUNT;

    /// Resolves a tag name, ignoring ASCII case.
    #[inline]
    pub const fn of(name: &str) -> Self {
        lookup(key(name.as_bytes()))
    }

    /// Dense index in `0..TagId::COUNT`, suitable for lookup tables.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Canonical lowercase name, or `""` for [`TagId::UNKNOWN`].
    #[inline]
    pub const fn name(self) -> &'static str {
        NAMES[self.0 as usize]
    }

    #[inline(always)]
    pub const fn is_known(self) -> bool {
        self.0 != Self::UNKNOWN.0
    }
}

const _: () = assert!(TAG_COUNT <= u8::MAX as usize, "tag ids must fit in u8");

#[cfg(test)]
mod tests {
    use super::{NAMES, TagId};

    #[test]
    fn every_known_name_round_trips() {
        for (index, name) in NAMES.iter().enumerate().skip(1) {
            let id = TagId::of(name);
            assert_eq!(id.index(), index, "{name}");
            assert_eq!(id.name(), *name);
            assert!(name.len() <= super::MAX_KEY_LEN, "{name}");
            assert_eq!(*name, name.to_ascii_lowercase(), "{name}");
        }
    }

    #[test]
    fn lookup_ignores_ascii_case_only() {
        assert_eq!(TagId::of("DIV"), TagId::DIV);
        assert_eq!(TagId::of("ClipPath"), TagId::CLIPPATH);
        assert_eq!(TagId::of("dİv"), TagId::UNKNOWN);
        // Bytes that differ from a known name only in bit 5 are not letters.
        assert_eq!(TagId::of("\x01"), TagId::UNKNOWN);
        assert_eq!(TagId::of("h\x11"), TagId::UNKNOWN);
        assert_eq!(TagId::of("@"), TagId::UNKNOWN);
    }

    #[test]
    fn lowercase_folds_only_ascii_letters() {
        for byte in 0..=u8::MAX {
            let word = u64::from(byte) << 24 | 0x5a40_5b41;
            let expected = u64::from_le_bytes(word.to_le_bytes().map(|b| b.to_ascii_lowercase()));
            assert_eq!(super::lowercase(word), expected, "{byte:#x}");
        }
    }

    #[test]
    fn unknown_names_share_one_id() {
        for name in [
            "",
            "my-widget",
            "di",
            "divv",
            "div\0",
            "a\0",
            "averyveryverylongtagname",
        ] {
            assert_eq!(TagId::of(name), TagId::UNKNOWN, "{name:?}");
        }
        assert!(!TagId::UNKNOWN.is_known());
        assert!(TagId::SPAN.is_known());
    }

    #[test]
    fn lookup_is_const() {
        const ARTICLE: TagId = TagId::of("article");
        assert_eq!(ARTICLE, TagId::ARTICLE);
    }
}
