use super::query_tokenizer::{Combinator, PatternStep};
use crate::css::parser::element::element::QueryElement;
use crate::utils::reader::Reader;

#[derive(PartialEq, Debug)]
pub enum Mode {
    First,
    All,
}

#[derive(PartialEq, Debug)]
pub struct PatternSection<'a> {
    pub(crate) pattern: Vec<PatternStep<'a>>,
    pub(crate) kind: Mode,

    pub(crate) parent: Option<usize>, // real index
    pub(crate) children: Vec<usize>,  // offset
}

impl<'a> PatternSection<'a> {
    pub fn new(reader: &mut Reader<'a>, mode: Mode) -> Self {
        let mut selection = Self {
            pattern: Vec::new(),
            kind: mode,
            parent: None,
            children: Vec::new(),
        };

        while let Some(token) = PatternStep::next(reader, selection.pattern.last()) {
            selection.pattern.push(token);
        }
        selection.pattern.push(PatternStep::Save);

        return selection;
    }

    pub fn len(&self) -> usize {
        return self.pattern.len();
    }
}

pub struct Pattern<'query> {
    descendants: Vec<Vec<usize>>,
    pub list: Vec<PatternSection<'query>>,
}

pub enum NextPosition {
    Link(usize, usize),
    Fork(Vec<(usize, usize)>),
}

impl<'query> Pattern<'query> {
    fn generate_descendants(&mut self) {
        for section_index in 0..self.list.len() {
            let mut section_descendants = Vec::new();
            let pattern_steps = &self.list[section_index].pattern;
            for index in 0..pattern_steps.len() {
                if pattern_steps[index] == PatternStep::Combinator(Combinator::Descendant) {
                    section_descendants.push(index);
                }
            }
            self.descendants.push(section_descendants);
        }
    }

    pub fn new(list: Vec<PatternSection<'query>>) -> Self {
        Self {
            descendants: Vec::new(),
            list: list,
        }
    }

    pub fn append(&mut self, mut sections: Vec<PatternSection<'query>>) -> () {
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
                    sections[index].parent = Some(list_len - 1);
                    last.children.push(index);
                }
            }
        }

        self.list.append(&mut sections);
    }

    pub fn next(&self, section_index: usize, step_index: usize) -> NextPosition {
        assert!(section_index < self.list.len());
        assert!(step_index < self.list[section_index].len());

        if step_index + 1 < self.list[section_index].len() {
            return NextPosition::Link(section_index, step_index + 1);
        } else {
            let ref section = self.list[section_index];
            return NextPosition::Fork(
                section
                    .children
                    .iter()
                    .map(|child_index| (*child_index, 0))
                    .collect(),
            );
        }
    }

    pub fn back(&self, section_index: usize, step_index: usize) -> (usize, usize) {
        assert!(section_index < self.list.len());
        assert!(step_index < self.list[section_index].len());

        if step_index > 0 {
            return (section_index, step_index - 1);
        } else {
            let ref section = self.list[section_index];
            if let Some(parent) = section.parent {
                return (parent, section.len() - 1);
            } else {
                return (0, 0);
            }
        }
    }

    pub fn last_descendant(
        &self,
        fsm_section_index: usize,
        fsm_step_index: usize,
    ) -> (usize, usize) {
        // BUG The pattern sections are not linkedlist it's a tree so in reality this need to check the parent
        for section_index in (0..fsm_section_index + 1).rev() {
            for step_index in (&self.descendants[section_index]).into_iter().rev() {
                if *step_index < fsm_step_index {
                    return (section_index, *step_index);
                }
            }
        }
        return (0, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::super::element::element::QueryElement;
    use super::super::pattern::{Mode, PatternSection};
    use super::super::query_tokenizer::Combinator;
    use super::*;
    use crate::utils::reader::Reader;

    #[test]
    fn test_selection_on_basic_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let section = PatternSection::new(&mut reader, Mode::All);
        let selection = Pattern::new(Vec::from([section]));

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
        let mut selection = Pattern::new(Vec::from([section]));

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
