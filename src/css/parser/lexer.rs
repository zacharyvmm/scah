use super::element::QueryElement;
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


impl<'a> Combinator {
    pub fn try_from(reader: &mut Reader<'a>) -> Option<Self> {
        let mut combinator: Option<Self> = None;
        while let Some(next_combinator) = Combinator::next(reader) {
            match combinator {
                Option::None => combinator = Some(next_combinator),
                Some(c) if c == Self::Descendant && next_combinator != Self::Descendant => {
                    combinator = Some(next_combinator);
                }
                _ => ()
            }
        }

        return combinator;
    }
}


pub struct Lexer {}
impl Lexer {
    pub fn next<'query>(reader: &mut Reader<'query>) -> (Combinator, QueryElement<'query>) {
        // if it doesn't start with a Combinator the default is Combinator:Descendant,
        // since a selector like `p` is technically a descendant search from the root, 
        // thus the actual query look like `%root% p`

        let combinator = Combinator::try_from(reader).unwrap_or(Combinator::Descendant);

        let element = QueryElement::from(reader);

        return (combinator, element);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_element_selection_with_combinator() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let (first_combinator, first_element) = Lexer::next(&mut reader);
        let (second_combinator, second_element) = Lexer::next(&mut reader);

        assert_eq!(first_combinator, Combinator::Descendant);

        assert_eq!(
            first_element,
            QueryElement::new(Some("element"), Some("id"), Some("class"), Vec::new(),)
        );

        assert_eq!(second_combinator, Combinator::Child);

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

    #[test]
    fn test_combinator_leading_selector() {
        let mut reader = Reader::new("~ element#id.class > other#other_id.other_class");
        let (first_combinator, first_element) = Lexer::next(&mut reader);
        let (second_combinator, second_element) = Lexer::next(&mut reader);

        assert_eq!(first_combinator, Combinator::SubsequentSibling);

        assert_eq!(
            first_element,
            QueryElement::new(Some("element"), Some("id"), Some("class"), Vec::new(),)
        );

        assert_eq!(second_combinator, Combinator::Child);

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
