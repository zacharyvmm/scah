use super::element::ElementPredicate;
use crate::utils::Reader;

#[derive(Debug, PartialEq, Clone)]
pub enum Combinator {
    // u4: Last Element Depth (size of stack)
    Child,       // `>`
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
            if !matches!(token, b'>' | b' ' | b'+' | b'~' | b'|') {
                return None;
            };
        }

        match reader.next()? {
            b'>' => Some(Self::Child),
            b' ' => Some(Self::Descendant),
            b'+' => Some(Self::NextSibling),
            b'~' => Some(Self::SubsequentSibling),
            b'|' => Some(Self::Namespace),
            _ => panic!("Not possible root"),
        }
    }
}

impl<'a> Combinator {
    pub fn try_from(reader: &mut Reader<'a>) -> Option<Self> {
        let mut combinator: Option<Self> = None;
        while let Some(next_combinator) = Combinator::next(reader) {
            match combinator {
                Option::None => combinator = Some(next_combinator),
                Some(c) if c == Self::Descendant && next_combinator != Self::Descendant => {
                    combinator = Some(next_combinator);
                }
                _ => (),
            }
        }

        return combinator;
    }

    pub(crate) fn is_descendant(&self) -> bool {
        *self == Self::Descendant
    }

    pub(crate) fn evaluate(&self, last_depth: u16, current_depth: u16) -> bool {
        match self {
            Combinator::Child => last_depth + 1 == current_depth,
            Combinator::Descendant => last_depth == 0 || current_depth != last_depth,

            // BUG: I need to know if it's the element right after
            // TODO: After first Fail it goes back
            Combinator::NextSibling => last_depth == current_depth,

            // BUG: I need to know if it's found a match before, so I know if it's ON/OFF
            Combinator::SubsequentSibling => true,

            Combinator::Namespace => panic!("Why are you using Namespace Selector ???"),
        }
    }
}

pub struct Lexer {}
impl Lexer {
    pub fn next<'query>(
        reader: &mut Reader<'query>,
    ) -> Option<(Combinator, ElementPredicate<'query>)> {
        if reader.eof() {
            return None;
        }

        // if it doesn't start with a Combinator the default is Combinator:Descendant,
        // since a selector like `p` is technically a descendant search from the root,
        // thus the actual query look like `%root% p`

        let combinator = Combinator::try_from(reader).unwrap_or(Combinator::Descendant);

        let element = ElementPredicate::from(reader);

        return Some((combinator, element));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_element_selection_with_combinator() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let (first_combinator, first_element) = Lexer::next(&mut reader).unwrap();
        let (second_combinator, second_element) = Lexer::next(&mut reader).unwrap();

        assert_eq!(first_combinator, Combinator::Descendant);

        assert_eq!(
            first_element,
            ElementPredicate {
                name: Some("element"),
                id: Some("id"),
                class: Some("class"),
                attributes: vec![]
            }
        );

        assert_eq!(second_combinator, Combinator::Child);

        assert_eq!(
            second_element,
            ElementPredicate {
                name: Some("other"),
                id: Some("other_id"),
                class: Some("other_class"),
                attributes: Vec::new(),
            }
        );
    }

    #[test]
    fn test_combinator_leading_selector() {
        let mut reader = Reader::new("~ element#id.class > other#other_id.other_class");
        let (first_combinator, first_element) = Lexer::next(&mut reader).unwrap();
        let (second_combinator, second_element) = Lexer::next(&mut reader).unwrap();

        assert_eq!(first_combinator, Combinator::SubsequentSibling);

        assert_eq!(
            first_element,
            ElementPredicate {
                name: Some("element"),
                id: Some("id"),
                class: Some("class"),
                attributes: vec![]
            }
        );

        assert_eq!(second_combinator, Combinator::Child);

        assert_eq!(
            second_element,
            ElementPredicate {
                name: Some("other"),
                id: Some("other_id"),
                class: Some("other_class"),
                attributes: Vec::new(),
            }
        );
    }
}
