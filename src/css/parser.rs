use crate::utils::reader::Reader;

enum AttributeSelectionKind {
    Presence,             // [attribute]
    Exact,                // [attribute=value]
    WhitespaceSeparated,  // [attribute~=value]
    HyphenSeparated,      // [attribute|=value]
    Prefix,               // [attribute^=value]
    Suffix,               // [attribute$=value]
    Substring,            // [attribute*=value]
}

enum CombinatorKind {
    Child,              // `>`
    Descendant,         // ` `
    NextSibling,        // `+`
    SubsequentSibling,  // `~`

    // I'm pretty sure this does not apply to the scope of the project.
    Namespace           // `|`
}

struct AttributeSelection<'a> {
    name: &'a str,
    value: Option<&'a str>,
    kind: AttributeSelectionKind,
}

struct Element<'a> {
    name: Option<&'a str>,
    id: Option<&'a str>,
    class: Option<&'a str>,
    attributes: Vec<AttributeSelection<'a>>,
}

enum QueryKind<'a> {
    Element(Element<'a>),

    Combinator(CombinatorKind),

    Has(Element<'a>),     // `:has()`
    Not(Element<'a>),     // `:not()`

    Or(Box<Self>),         // This is the `,` on a selection. ex: `a#hello > p, p.world`
}

struct Selection<'a> {
    query: Vec<QueryKind<'a>>,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fully_detailed_element_selection() {
        let reader = Reader::new("element#id.class[selected=true]");
    }

    #[test]
    fn test_handle_duplicates_in_element_definition() {
        let reader = Reader::new("element#id.class[selected=true]#id#notid");
        // Since this is used by the developer is acceptable to throw an error in the system
    }
}