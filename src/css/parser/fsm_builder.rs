use super::pattern_section::PatternSection;
use super::query_tokenizer::PatternStep;
use crate::css::parser::element::element::QueryElement;

pub struct FsmBuilder<'a> {
    pub list: Vec<PatternSection<'a>>,
}

impl<'a> FsmBuilder<'a> {
    pub fn new(sections: Vec<PatternSection<'a>>) -> Self {
        Self { list: sections }
    }

    pub fn append(&mut self, mut sections: Vec<PatternSection<'a>>) -> () {
        let list_len = self.list.len();
        if list_len > 0 && sections.len() > 1 {
            let ref mut last = self.list[list_len - 1];

            for index in 0..sections.len() {
                // ----- Empty element -----
                assert!(matches!(
                    sections[index].pattern[0],
                    PatternStep::Element(QueryElement {
                        name: None | Some("&"),
                        ..
                    })
                ));
                sections[index].pattern.remove(0);
                // -------------------------

                if sections[index].parent == Option::None {
                    sections[index].parent = Some((list_len - 1) as u16);
                    last.children.push(index as u16);
                }
            }
        }

        self.list.append(&mut sections);
    }
}

#[cfg(test)]
mod tests {
    use super::super::element::element::QueryElement;
    use super::super::pattern_section::{Mode, PatternSection};
    use super::super::query_tokenizer::Combinator;
    use super::*;
    use crate::utils::reader::Reader;

    #[test]
    fn test_selection_on_basic_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let section = PatternSection::new(&mut reader, Mode::All);
        let selection = FsmBuilder::new(Vec::from([section]));

        assert_eq!(
            selection.list[0].pattern,
            Vec::from([
                PatternStep::Element(QueryElement::new(
                    Some("element"),
                    Some("id"),
                    Some("class"),
                    Vec::new(),
                )),
                PatternStep::Combinator(Combinator::Child),
                PatternStep::Element(QueryElement::new(
                    Some("other"),
                    Some("other_id"),
                    Some("other_class"),
                    Vec::new(),
                )),
                PatternStep::Save,
            ])
        );
    }

    #[test]
    fn test_selection_on_complex_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let mut then_a_reader = Reader::new("> a");
        let mut then_p_reader = Reader::new(" p");

        let section = PatternSection::new(&mut reader, Mode::All);
        let mut selection = FsmBuilder::new(Vec::from([section]));

        assert_eq!(
            selection.list,
            Vec::from([PatternSection {
                pattern: Vec::from([
                    PatternStep::Element(QueryElement::new(
                        Some("element"),
                        Some("id"),
                        Some("class"),
                        Vec::new(),
                    )),
                    PatternStep::Combinator(Combinator::Child),
                    PatternStep::Element(QueryElement::new(
                        Some("other"),
                        Some("other_id"),
                        Some("other_class"),
                        Vec::new(),
                    )),
                    PatternStep::Save,
                ]),
                kind: Mode::All,
                parent: Option::None,
                children: Vec::new(),
            }])
        );

        selection.append(Vec::from([
            PatternSection::new(&mut then_a_reader, Mode::All),
            PatternSection::new(&mut then_p_reader, Mode::All),
        ]));

        assert_eq!(
            selection.list,
            Vec::from([
                PatternSection {
                    pattern: Vec::from([
                        PatternStep::Element(QueryElement::new(
                            Some("element"),
                            Some("id"),
                            Some("class"),
                            Vec::new(),
                        )),
                        PatternStep::Combinator(Combinator::Child),
                        PatternStep::Element(QueryElement::new(
                            Some("other"),
                            Some("other_id"),
                            Some("other_class"),
                            Vec::new(),
                        )),
                        PatternStep::Save,
                    ]),
                    kind: Mode::All,
                    parent: Option::None,
                    children: Vec::from([0, 1]),
                },
                PatternSection {
                    pattern: Vec::from([
                        PatternStep::Combinator(Combinator::Child),
                        PatternStep::Element(QueryElement::new(Some("a"), None, None, Vec::new(),)),
                        PatternStep::Save,
                    ]),
                    kind: Mode::All,
                    parent: Some(0),
                    children: Vec::new(),
                },
                PatternSection {
                    pattern: Vec::from([
                        PatternStep::Combinator(Combinator::Descendant),
                        PatternStep::Element(QueryElement::new(Some("p"), None, None, Vec::new(),)),
                        PatternStep::Save,
                    ]),
                    kind: Mode::All,
                    parent: Some(0),
                    children: Vec::new(),
                },
            ])
        );
    }
}
