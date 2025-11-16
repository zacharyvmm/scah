use std::ops::Index;
use std::collections::HashMap;


#[derive(Debug, PartialEq)]
pub enum ItemOrList<'query, 'html> {
    One(Element<'query, 'html>),
    List(Vec<Element<'query, 'html>>),
    None,
}

impl<'query, 'html> Index<&str> for ItemOrList<'query, 'html> {
    type Output = ItemOrList<'query, 'html>;

    fn index(&self, key: &str) -> &Self::Output {
        match self {
            ItemOrList::One(el) => &el[key],
            _ => panic!("Cannot use a key on this item")
        }
    }
}

impl<'query, 'html> Index<usize> for ItemOrList<'query, 'html> {
    type Output = Element<'query, 'html>;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            ItemOrList::List(list) => &list[index],
            _ => panic!("Cannot use an index on this item")
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Element<'query, 'html> {
    pub name: &'html str,
    pub class: Option<&'html str>,
    pub id: Option<&'html str>,
    pub attributes: Vec<(&'html str, &'html str)>,
    pub inner_html: Option<&'html str>,
    pub text_content: Option<&'html str>,
    children: Vec<(&'query str, ItemOrList<'query, 'html>)>,
}

impl<'query, 'html> ItemOrList<'query, 'html> {
    pub fn name(&self) -> &'html str {
        match self {
            Self::One(element) => element.name,
            _ => panic!("Cannot access Element")
        }
    } 

    pub fn class(&self) -> Option<&'html str> {
        match self {
            Self::One(element) => element.class,
            _ => panic!("Cannot access Element")
        }
    } 

    pub fn id(&self) -> Option<&'html str> {
        match self {
            Self::One(element) => element.id,
            _ => panic!("Cannot access Element")
        }
    } 
    
    pub fn attributes(&self) -> &Vec<(&'html str, &'html str)> {
        match self {
            Self::One(element) => &element.attributes,
            _ => panic!("Cannot access Element")
        }
    } 

    pub fn inner_html(&self) -> Option<&'html str> {
        match self {
            Self::One(element) => element.inner_html,
            _ => panic!("Cannot access Element")
        }
    } 

    pub fn text_content(&self) -> Option<&'html str> {
        match self {
            Self::One(element) => element.text_content,
            _ => panic!("Cannot access Element")
        }
    } 
}

impl<'query, 'html> Index<&str> for Element<'query, 'html> {
    type Output = ItemOrList<'query, 'html>;
    fn index(&self, key: &str) -> &Self::Output {
        self.children
            .iter()
            .find(|(k, _)| k == &key)
            .map(|(_, v)| v)
            .unwrap_or(&ItemOrList::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_access() {
        let mut selectors: HashMap<&str, Element> = HashMap::new();
        selectors.insert(
            "div > p.indent",
            Element {
                name: "p",
                class: Some("indent"),
                id: Some("hello"),
                attributes: vec![("hello", "world")],
                inner_html: None,
                text_content: None,
                children: vec![(
                    "a",
                    ItemOrList::List(vec![Element {
                        name: "p",
                        class: Some("indent"),
                        id: Some("hello"),
                        attributes: vec![("hello", "world")],
                        inner_html: None,
                        text_content: None,
                        children: vec![],
                    }]),
                )],
            },
        );

        assert_eq!(selectors["div > p.indent"]["a"],
                    ItemOrList::List(vec![Element {
                        name: "p",
                        class: Some("indent"),
                        id: Some("hello"),
                        attributes: vec![("hello", "world")],
                        inner_html: None,
                        text_content: None,
                        children: vec![],
                    }]),
        );
    }
}
