//! Differential testing of the current engine against the last release.
//!
//! [`Case::generate`] builds a random, often malformed, HTML document and a
//! random query set (combinators, selector lists, pseudo-classes, nested
//! `all`/`first` sections, and every `Save` mode). [`run_current`] and
//! [`run_reference`] parse it with this checkout and with the published
//! `scah`, and render every result tree to the same text form, so any
//! semantic difference shows up as a text diff.

use std::fmt::Write as _;

/// Deterministic `splitmix64` generator, so failing seeds reproduce.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const TAGS: &[&str] = &[
    "div", "p", "a", "span", "ul", "li", "section", "h1", "em", "b", "article", "main", "table",
    "tr", "td", "dl", "dt", "dd", "option", "pre",
];
const VOID_TAGS: &[&str] = &["br", "img", "hr", "input"];
const IDS: &[&str] = &["a", "b", "c", "x"];
const CLASSES: &[&str] = &["x", "y", "x y", "z", "y z"];
const TEXT: &[&str] = &[
    "t",
    "Hello world",
    "a &amp; b",
    "  ",
    "\n",
    "x&lt;y",
    "one\ntwo",
];

/// Which content a section keeps for its matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    All,
    None,
    NameOnly,
    OnlyText,
    OnlyInnerHtml,
    OnlyRawText,
}

const SAVES: &[SaveKind] = &[
    SaveKind::All,
    SaveKind::None,
    SaveKind::NameOnly,
    SaveKind::OnlyText,
    SaveKind::OnlyInnerHtml,
    SaveKind::OnlyRawText,
];

/// One query section and its nested sections.
#[derive(Debug, Clone)]
pub struct SectionSpec {
    pub selector: &'static str,
    pub first: bool,
    pub save: SaveKind,
    pub children: Vec<SectionSpec>,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub html: String,
    pub queries: Vec<SectionSpec>,
}

impl Case {
    pub fn generate(rng: &mut Rng) -> Self {
        let mut html = String::new();
        let top = 2 + rng.below(5);
        for _ in 0..top {
            generate_node(rng, &mut html, 0);
        }
        // End with text: the EOF drain of documents that end right after a
        // tag differs between releases on purpose (#93).
        html.push_str("end");

        let queries = (0..1 + rng.below(3))
            .map(|_| generate_section(rng, false, 0))
            .collect();
        Self { html, queries }
    }

    pub fn describe(&self) -> String {
        let mut out = format!("html: {:?}\nqueries:\n", self.html);
        for query in &self.queries {
            describe_section(query, 1, &mut out);
        }
        out
    }
}

fn describe_section(section: &SectionSpec, indent: usize, out: &mut String) {
    let _ = writeln!(
        out,
        "{}{}({:?}, {:?})",
        "  ".repeat(indent),
        if section.first { "first" } else { "all" },
        section.selector,
        section.save
    );
    for child in &section.children {
        describe_section(child, indent + 1, out);
    }
}

fn generate_node(rng: &mut Rng, html: &mut String, depth: usize) {
    if rng.chance(25) {
        html.push_str(rng.pick(TEXT));
        return;
    }
    if rng.chance(12) {
        let tag = rng.pick(VOID_TAGS);
        let _ = write!(html, "<{tag}");
        generate_attributes(rng, html);
        html.push('>');
        return;
    }
    let tag = rng.pick(TAGS);
    let _ = write!(html, "<{tag}");
    generate_attributes(rng, html);
    html.push('>');
    if depth < 6 {
        for _ in 0..rng.below(5) {
            generate_node(rng, html, depth + 1);
        }
    }
    // Leave some elements open, and add the odd stray or misnested close tag.
    if !rng.chance(15) {
        let _ = write!(html, "</{tag}>");
    }
    if rng.chance(4) {
        let _ = write!(html, "</{}>", rng.pick(TAGS));
    }
}

fn generate_attributes(rng: &mut Rng, html: &mut String) {
    if rng.chance(30) {
        let _ = write!(html, " id=\"{}\"", rng.pick(IDS));
    }
    if rng.chance(40) {
        let _ = write!(html, " class=\"{}\"", rng.pick(CLASSES));
    }
    if rng.chance(20) {
        let _ = write!(
            html,
            " href=\"{}\"",
            rng.pick(&["/a", "https://e.com/x", "#top"])
        );
    }
    if rng.chance(10) {
        html.push_str(" data-k=v");
    }
    if rng.chance(5) {
        html.push_str(" hidden");
    }
}

fn generate_compound(rng: &mut Rng) -> String {
    let mut compound = String::new();
    let mut has_simple = false;
    if rng.chance(85) {
        compound.push_str(rng.pick(&[
            "div", "div", "p", "p", "a", "span", "ul", "li", "li", "section", "td", "br", "*",
        ]));
        has_simple = true;
    }
    if rng.chance(8) {
        let _ = write!(compound, "#{}", rng.pick(IDS));
        has_simple = true;
    }
    if rng.chance(20) {
        let _ = write!(compound, ".{}", rng.pick(&["x", "y", "z"]));
        has_simple = true;
    }
    if rng.chance(8) {
        compound.push_str(rng.pick(&[
            "[href]",
            "[href^=\"https\"]",
            "[data-k=v]",
            "[class~=y]",
            "[href$=\"/a\" i]",
        ]));
        has_simple = true;
    }
    if rng.chance(10) {
        compound.push_str(rng.pick(&[
            ":first-child",
            ":nth-child(2n+1)",
            ":nth-child(-n+2)",
            ":first-of-type",
            ":nth-of-type(2)",
            ":not(.x)",
            ":is(p, li)",
            ":nth-child(odd of .x)",
            ":root",
        ]));
        has_simple = true;
    }
    if !has_simple {
        compound.push_str(rng.pick(&["div", "p", "a"]));
    }
    compound
}

fn generate_complex(rng: &mut Rng, scoped: bool) -> String {
    let mut selector = String::new();
    if scoped && rng.chance(35) {
        selector.push_str("> ");
    }
    selector.push_str(&generate_compound(rng));
    for _ in 0..rng.below(3) {
        selector.push_str(rng.pick(&[" ", " ", " > ", " > ", " + ", " ~ "]));
        selector.push_str(&generate_compound(rng));
    }
    selector
}

fn generate_section(rng: &mut Rng, scoped: bool, depth: usize) -> SectionSpec {
    let mut selector = generate_complex(rng, scoped);
    if rng.chance(15) {
        selector.push_str(", ");
        selector.push_str(&generate_complex(rng, scoped));
    }
    let children = if depth < 2 && rng.chance(50) {
        (0..1 + rng.below(2))
            .map(|_| generate_section(rng, true, depth + 1))
            .collect()
    } else {
        Vec::new()
    };
    SectionSpec {
        selector: Box::leak(selector.into_boxed_str()),
        first: rng.chance(30),
        save: SAVES[rng.below(SAVES.len())],
        children,
    }
}

macro_rules! engine {
    ($name:ident, $krate:ident) => {
        /// Parse `case` and render every result tree, or `None` when this
        /// engine rejects one of the selectors.
        pub fn $name(case: &Case) -> Option<String> {
            use $krate::{Query, QueryBuilder, QueryFactory, Save};

            fn save(kind: SaveKind) -> Save {
                match kind {
                    SaveKind::All => Save::all(),
                    SaveKind::None => Save::none(),
                    SaveKind::NameOnly => Save::name_only(),
                    SaveKind::OnlyText => Save::only_text(),
                    SaveKind::OnlyInnerHtml => Save::only_inner_html(),
                    SaveKind::OnlyRawText => Save::only_raw_text(),
                }
            }

            fn with_children(
                builder: QueryBuilder<'static>,
                section: &SectionSpec,
            ) -> Result<QueryBuilder<'static>, $krate::SelectorParseError> {
                if section.children.is_empty() {
                    return Ok(builder);
                }
                builder.then(|factory: QueryFactory| {
                    section
                        .children
                        .iter()
                        .map(|child| {
                            let builder = if child.first {
                                factory.first(child.selector, save(child.save))?
                            } else {
                                factory.all(child.selector, save(child.save))?
                            };
                            with_children(builder, child)
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
            }

            fn render(
                store: &$krate::Store,
                element: &$krate::Element,
                section: &SectionSpec,
                out: &mut String,
            ) {
                let _ = write!(out, "<{}", element.name);
                if let Some(id) = element.id {
                    let _ = write!(out, " id={id:?}");
                }
                if let Some(class) = element.class {
                    let _ = write!(out, " class={class:?}");
                }
                for attribute in element.attributes(store).unwrap_or_default() {
                    let _ = write!(out, " {}={:?}", attribute.key, attribute.value);
                }
                out.push('>');
                if let Some(inner_html) = element.inner_html {
                    let _ = write!(out, " inner_html={inner_html:?}");
                }
                if let Some(raw_text) = element.raw_text(store) {
                    let _ = write!(out, " raw_text={raw_text:?}");
                }
                if let Some(text) = element.text(store) {
                    let _ = write!(out, " text={text:?}");
                }
                for child in &section.children {
                    let _ = write!(out, " {{{}:", child.selector);
                    if let Some(elements) = element.get(store, child.selector) {
                        for nested in elements {
                            out.push(' ');
                            render(store, nested, child, out);
                        }
                    }
                    out.push('}');
                }
            }

            let mut queries = Vec::new();
            for section in &case.queries {
                let builder = if section.first {
                    Query::first(section.selector, save(section.save)).ok()?
                } else {
                    Query::all(section.selector, save(section.save)).ok()?
                };
                queries.push(with_children(builder, section).ok()?.build());
            }
            let html: &'static str = Box::leak(case.html.clone().into_boxed_str());
            let queries: &'static [Query<'static>] = Box::leak(queries.into_boxed_slice());
            let store = $krate::parse(html, queries).expect("parse succeeds");

            let mut out = String::new();
            for section in &case.queries {
                let _ = write!(out, "{}:", section.selector);
                if let Some(elements) = store.get(section.selector) {
                    for element in elements {
                        out.push_str("\n  ");
                        render(&store, element, section, &mut out);
                    }
                }
                out.push('\n');
            }
            Some(out)
        }
    };
}

engine!(run_current, scah);
engine!(run_reference, scah_reference);
