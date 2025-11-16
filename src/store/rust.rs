use super::traits::Store;
use std::collections::HashMap;
use std::ops::Index;

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError<'key> {
    KeyNotFound(&'key str),
    NotASingleElement,
    NotAList,
    IndexOutOfBounds { index: usize, len: usize },
}

#[derive(Debug, PartialEq)]
pub enum SelectionValue<'html, 'query> {
    One(Box<Element<'html, 'query>>),
    Many(Vec<Element<'html, 'query>>),
}

impl<'html, 'query, 'key> SelectionValue<'html, 'query> {
    fn one(&self) -> Result<&Element<'html, 'query>, QueryError<'key>> {
        match self {
            SelectionValue::One(el) => Ok(el),
            SelectionValue::Many(_) => Err(QueryError::NotASingleElement),
        }
    }

    fn list(&'html self) -> Result<&'html [Element<'html, 'query>], QueryError<'key>> {
        match self {
            SelectionValue::Many(vec) => Ok(vec),
            SelectionValue::One(_) => Err(QueryError::NotAList),
        }
    }

    /// Field accessors (only work on single elements)
    /// I don't know how I feel about this; it could this cause confusion.
    pub fn name(&'html self) -> Result<&'html str, QueryError<'key>> {
        self.one().map(|el| el.name)
    }

    pub fn class(&'html self) -> Result<Option<&'html str>, QueryError<'key>> {
        self.one().map(|el| el.class)
    }

    pub fn id(&'html self) -> Result<Option<&'html str>, QueryError<'key>> {
        self.one().map(|el| el.id)
    }

    pub fn attributes(&'html self) -> Result<&'html [(&'html str, &'html str)], QueryError<'key>> {
        self.one().map(|el| el.attributes.as_slice())
    }

    pub fn inner_html(&'html self) -> Result<Option<&'html str>, QueryError<'key>> {
        self.one().map(|el| el.inner_html)
    }

    pub fn text_content(&'html self) -> Result<Option<&'html str>, QueryError<'key>> {
        self.one().map(|el| el.text_content)
    }

    // Solution to field accessor confusion
    #[inline]
    pub fn value(&self) -> Result<&Element<'html, 'query>, QueryError<'key>> {
        self.one()
    }

    /// List operations
    pub fn iter(
        &'html self,
    ) -> Result<impl Iterator<Item = &'html Element<'html, 'query>>, QueryError<'key>> {
        self.list().map(|vec| vec.iter())
    }

    pub fn len(&'html self) -> Result<usize, QueryError<'key>> {
        self.list().map(|vec| vec.len())
    }

    pub fn get(
        &'html self,
        index: usize,
    ) -> Result<&'html Element<'html, 'query>, QueryError<'key>> {
        self.list()?
            .get(index)
            .ok_or_else(|| QueryError::IndexOutOfBounds {
                index,
                len: self.list().unwrap().len(),
            })
    }
}

#[derive(Debug, PartialEq)]
pub struct Element<'html, 'query> {
    pub name: &'html str,
    pub class: Option<&'html str>,
    pub id: Option<&'html str>,
    pub attributes: Vec<(&'html str, &'html str)>,
    pub inner_html: Option<&'html str>,
    pub text_content: Option<&'html str>,
    // Store Selection directly to enable Index trait
    children: HashMap<&'query str, SelectionValue<'html, 'query>>,
}

impl<'html, 'query, 'key> Element<'html, 'query> {
    /// Safe primary access method
    pub fn get(
        &'html self,
        key: &'key str,
    ) -> Result<&'html SelectionValue<'html, 'query>, QueryError<'key>> {
        self.children.get(key).ok_or(QueryError::KeyNotFound(key))
    }

    /// Panicking accessor for known keys
    pub fn select(&'html self, key: &'key str) -> &'html SelectionValue<'html, 'query> {
        self.get(key).unwrap()
    }

    /// Check existence without error
    pub fn contains_key(&self, key: &'key str) -> bool {
        self.children.contains_key(key)
    }
}

impl<'html, 'query> Index<&'query str> for Element<'html, 'query> {
    type Output = SelectionValue<'html, 'query>;

    fn index(&self, key: &'query str) -> &Self::Output {
        &self.children[key] // Panics if key not found
    }
}

impl<'html, 'query> Index<usize> for SelectionValue<'html, 'query> {
    type Output = Element<'html, 'query>;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            SelectionValue::Many(vec) => &vec[index], // Panics if out of bounds
            SelectionValue::One(_) => panic!("Cannot use usize index on single element"),
        }
    }
}

impl<'html, 'query> Index<&'query str> for SelectionValue<'html, 'query> {
    type Output = SelectionValue<'html, 'query>;

    fn index(&self, key: &'query str) -> &Self::Output {
        match self {
            SelectionValue::One(boxed_el) => &boxed_el[key], // Panics if key not found
            SelectionValue::Many(_) => panic!("Cannot chain string index on list selection"),
        }
    }
}

pub struct ElementBuilder<'html, 'query> {
    name: &'html str,
    class: Option<&'html str>,
    id: Option<&'html str>,
    attributes: Vec<(&'html str, &'html str)>,
    inner_html: Option<&'html str>,
    text_content: Option<&'html str>,
    children: HashMap<&'query str, SelectionValue<'html, 'query>>,
}

impl<'html, 'query> ElementBuilder<'html, 'query> {
    pub fn new(name: &'html str) -> Self {
        Self {
            name,
            class: None,
            id: None,
            attributes: Vec::new(),
            inner_html: None,
            text_content: None,
            children: HashMap::new(),
        }
    }

    pub fn class(mut self, class: &'html str) -> Self {
        self.class = Some(class);
        self
    }

    pub fn id(mut self, id: &'html str) -> Self {
        self.id = Some(id);
        self
    }

    pub fn attribute(mut self, name: &'html str, value: &'html str) -> Self {
        self.attributes.push((name, value));
        self
    }

    pub fn inner_html(mut self, html: &'html str) -> Self {
        self.inner_html = Some(html);
        self
    }

    pub fn text_content(mut self, text: &'html str) -> Self {
        self.text_content = Some(text);
        self
    }

    /// Add a single child
    pub fn child(mut self, key: &'query str, element: Element<'html, 'query>) -> Self {
        self.children
            .insert(key, SelectionValue::One(Box::new(element)));
        self
    }

    /// Add multiple children
    pub fn children(mut self, key: &'query str, elements: Vec<Element<'html, 'query>>) -> Self {
        self.children.insert(key, SelectionValue::Many(elements));
        self
    }

    pub fn build(self) -> Element<'html, 'query> {
        Element {
            name: self.name,
            class: self.class,
            id: self.id,
            attributes: self.attributes,
            inner_html: self.inner_html,
            text_content: self.text_content,
            children: self.children,
        }
    }
}

struct RustStore<'html, 'query> {
    list: Vec<Element<'html, 'query>>,
}

impl<'html, 'query> Store<'html, Element<'html, 'query>> for RustStore<'html, 'query> {
    fn push(
        &mut self,
        from: &'html Element<'html, 'query>,
        element: crate::XHtmlElement<'html>,
    ) -> &Element<'html, 'query> {
        self.list.push(Element {
            name: element.name,
            class: element.class,
            id: element.id,
            attributes: vec![],
            inner_html: None,
            text_content: None,
            children: HashMap::new(),
        });
        let new_element_ref = self.list.last().unwrap();

        // attache new element to from element
        // from.children.insert(k, v)

        return new_element_ref;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_access() -> Result<(), QueryError<'static>> {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello").build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First").build(),
                    ElementBuilder::new("li").text_content("Second").build(),
                ],
            )
            .build();

        // Different ways of acessing fields
        assert_eq!(doc.get("title")?.name()?, "h1");
        assert_eq!(doc["title"].name()?, "h1");
        assert_eq!(doc.get("title")?.value()?.name, "h1");
        assert_eq!(doc["title"].value()?.name, "h1");

        assert_eq!(doc.get("items")?.len()?, 2);
        assert_eq!(doc["items"].len()?, 2);

        assert_eq!(doc.get("items")?.get(0)?.text_content, Some("First"));
        assert_eq!(doc["items"][0].text_content, Some("First"));

        // Iterators for All Selections
        let items_iter1 = doc.get("items")?.iter()?;
        assert_eq!(
            items_iter1.collect::<Vec<&Element>>(),
            vec![
                &ElementBuilder::new("li").text_content("First").build(),
                &ElementBuilder::new("li").text_content("Second").build(),
            ]
        );

        let items_iter2 = doc["items"].iter()?;
        assert_eq!(
            items_iter2.collect::<Vec<&Element>>(),
            vec![
                &ElementBuilder::new("li").text_content("First").build(),
                &ElementBuilder::new("li").text_content("Second").build(),
            ]
        );

        assert!(!doc.contains_key("optional"));

        assert_eq!(
            doc.get("optional"),
            Err(QueryError::KeyNotFound("optional"))
        );

        Ok(())
    }

    #[test]
    #[should_panic(expected = "no entry found for key")]
    fn test_non_existing_key_element_access() {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello").build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First").build(),
                    ElementBuilder::new("li").text_content("Second").build(),
                ],
            )
            .build();

        assert!(!doc.contains_key("non-existing"));
        assert_eq!(
            doc.get("non-existing"),
            Err(QueryError::KeyNotFound("non-existing"))
        );

        let non_existing = &doc["non-existing"];
    }

    #[test]
    #[should_panic(expected = "Cannot use usize index on single element")]
    fn test_index_on_single_element_access() {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello").build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First").build(),
                    ElementBuilder::new("li").text_content("Second").build(),
                ],
            )
            .build();

        assert_eq!(doc.get("title").unwrap().get(0), Err(QueryError::NotAList));

        let non_existing_list = &doc["title"][0];
    }

    #[test]
    #[should_panic(expected = "Cannot chain string index on list selection")]
    fn test_key_on_vec_element_access() {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello").build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First").build(),
                    ElementBuilder::new("li").text_content("Second").build(),
                ],
            )
            .build();

        // No Safe equivalent because of the types

        let non_existing_key = &doc["items"]["li"];
    }
}
