use super::element::Element;
use crate::utils::reader::Reader;

pub enum CombinatorKind {
    Child,             // `>`
    Descendant,        // ` `
    NextSibling,       // `+`
    SubsequentSibling, // `~`

    // I'm pretty sure this does not apply to the scope of the project.
    Namespace, // `|`
}
pub enum QueryKind<'a> {
    Element(Element<'a>),

    Combinator(CombinatorKind),

    Has(Element<'a>), // `:has()`
    Not(Element<'a>), // `:not()`

    Or(Box<Self>), // This is the `,` on a selection. ex: `a#hello > p, p.world`
}

pub struct Selection<'a> {
    query: Vec<QueryKind<'a>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_element_selection_with_combinator() {
        let mut reader = Reader::new("element#id.class");
        let mut element = Element::new();
        element.build(&mut reader);

        assert_eq!(
            element,
            Element {
                name: Some("element"),
                id: Some("id"),
                class: Some("class"),
                attributes: Vec::new(),
            }
        );
    }
}
