use super::element::element::QueryElement;
use crate::utils::reader::Reader;

#[derive(Debug, PartialEq, Clone)]
pub enum Combinator {
    // u4: Last Element Depth (size of stack)
    Child(u8),   // `>`
    Descendant,  // ` `
    NextSibling, // `+`

    // BUG: By definition of this Combinator it's a SelectAll query
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
            '>' => Some(Self::Child(0)),
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

#[derive(Debug, PartialEq, Clone)]
pub enum QueryKind<'a> {
    Element(QueryElement<'a>),

    Combinator(Combinator),

    Has(QueryElement<'a>), // `:has()`
    Not(QueryElement<'a>), // `:not()`

    // TODO: I will need to optimize away inoficient `Any` usage, ex: `p > * a` to `p  a`
    // Valid usage: `p > * > a`
    Any, // `*`

    // TODO: I'm not sure how this would belong to `QueryKind` and not `Selection`
    Or(Box<Self>), // This is the `,` on a selection. ex: `a#hello > p, p.world`

    EOF,
}

impl<'a> QueryKind<'a> {
    pub fn next(reader: &mut Reader<'a>, last: Option<&Self>) -> Option<Self> {
        match last {
            Option::None | Some(Self::Combinator(_)) => {
                Some(Self::Element(QueryElement::from(reader)))
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_element_selection_with_combinator() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let first_element = QueryElement::from(&mut reader);

        let combinator = Combinator::from(&mut reader);

        let second_element = QueryElement::from(&mut reader);

        assert_eq!(
            first_element,
            QueryElement::new(Some("element"), Some("id"), Some("class"), Vec::new(),)
        );

        assert_eq!(combinator, Combinator::Child(0));

        assert_eq!(
            second_element,
            QueryElement::new(
                Some("other"),
                Some("other_id"),
                Some("other_class"),
                Vec::new(),
            )
        );
    }
}
