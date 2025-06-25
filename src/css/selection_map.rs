use crate::xhtml::parser::BodyContent;

type SelectCollection<'html> = Vec<BodyContent<'html>>;
type SelectElement<'html> = BodyContent<'html>;

#[derive(Debug, PartialEq)]
enum Select<'html> {
    All(SelectCollection<'html>),
    One(SelectElement<'html>),
}

pub struct SelectionMap<'query, 'html> {
    selections: Vec<(&'query str, Select<'html>)>,
}

impl<'query, 'html> SelectionMap<'query, 'html> {
    pub(crate) fn new() -> Self {
        Self {
            selections: Vec::new(),
        }
    }

    pub(crate) fn create_pairing(&mut self, query: &'query str, selection: Select<'html>) -> usize {
        self.selections.push((query, selection));
        return self.selections.len() - 1;
    }

    pub(crate) fn append(&mut self, index: usize, content: SelectElement<'html>) -> () {
        if index >= self.selections.len() {
            return;
        }

        if let Select::All(query_results) = &mut self.selections[index].1 {
            query_results.push(content);
        } else {
            panic!("Selection set to a single element, but tried to append an element");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_() {
    }
}