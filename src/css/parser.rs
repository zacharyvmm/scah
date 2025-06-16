use super::element::Element;
use crate::utils::reader::Reader;

#[derive(Debug, PartialEq)]
pub enum Combinator {
    Child,             // `>`
    Descendant,        // ` `
    NextSibling,       // `+`
    SubsequentSibling, // `~`

    // I'm pretty sure this does not apply to the scope of the project.
    Namespace, // `|`
}

impl Combinator {
    fn next<'a>(reader: &mut Reader<'a>) -> Option<Self> {
        if let Some(token) = reader.peek() {
            if !matches!(token, '>' | ' ' | '+' | '~' | '|') {
                return None;
            };
        }

        match reader.next()? {
            '>' => Some(Self::Child),
            ' ' => Some(Self::Descendant),
            '+' => Some(Self::NextSibling),
            '~' => Some(Self::SubsequentSibling),
            '|' => Some(Self::Namespace),
            _ => panic!("Not possible root"),
        }
    }
}

impl<'a> From<&mut Reader<'a>> for Combinator {
    fn from(reader: &mut Reader<'a>) -> Self {
        let mut combinator: Option<Self> = None;
        while let Some(next_combinator) = Combinator::next(reader) {
            match combinator {
                Option::None => combinator = Some(next_combinator),
                Some(c) if c == Self::Descendant && next_combinator != Self::Descendant => {
                    combinator = Some(next_combinator);
                }
                _ => {}
            }
        }

        return combinator.expect("A combinator should be here");
    }
}

#[derive(Debug, PartialEq)]
pub enum QueryKind<'a> {
    Element(Element<'a>),

    Combinator(Combinator),

    Has(Element<'a>), // `:has()`
    Not(Element<'a>), // `:not()`

    // TODO: I'm not sure how this would belong to `QueryKind` and not `Selection`
    Or(Box<Self>), // This is the `,` on a selection. ex: `a#hello > p, p.world`
}

impl<'a> QueryKind<'a> {
    fn next(reader: &mut Reader<'a>, last: Option<&Self>) -> Option<Self> {
        match last {
            Option::None | Some(Self::Combinator(_)) => Some(Self::Element(Element::from(reader))),
            Some(_) => {
                if let Some(token) = reader.peek() {
                    if matches!(token, '>' | ' ' | '+' | '~' | '|') {
                        return Some(Self::Combinator(Combinator::from(reader)));
                    };
                }

                return None;
            }
        }
    }
}

pub struct Selection<'a> {
    query: Vec<QueryKind<'a>>,
}

impl<'a> From<&mut Reader<'a>> for Selection<'a> {
    fn from(reader: &mut Reader<'a>) -> Self {
        let mut selection = Self { query: Vec::new() };

        while let Some(query) = QueryKind::next(reader, selection.query.last()) {
            selection.query.push(query);
        }

        return selection;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_element_selection_with_combinator() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let first_element = Element::from(&mut reader);

        let combinator = Combinator::from(&mut reader);

        let second_element = Element::from(&mut reader);

        assert_eq!(
            first_element,
            Element::new(Some("element"), Some("id"), Some("class"), Vec::new(),)
        );

        assert_eq!(combinator, Combinator::Child);

        assert_eq!(
            second_element,
            Element::new(
                Some("other"),
                Some("other_id"),
                Some("other_class"),
                Vec::new(),
            )
        );
    }

    #[test]
    fn test_selection_on_basic_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let selection = Selection::from(&mut reader);

        assert_eq!(
            selection.query[0],
            QueryKind::Element(Element::new(
                Some("element"),
                Some("id"),
                Some("class"),
                Vec::new(),
            ))
        );

        assert_eq!(selection.query[1], QueryKind::Combinator(Combinator::Child));

        assert_eq!(
            selection.query[2],
            QueryKind::Element(Element::new(
                Some("other"),
                Some("other_id"),
                Some("other_class"),
                Vec::new(),
            ))
        );
    }
}
