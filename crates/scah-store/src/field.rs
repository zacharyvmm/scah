//! Rows read as byte ranges, for consumers that address values as offsets
//! into the store's buffers instead of copying them, such as scah-ffi's
//! Arrow export.
//!
//! Every string a store holds is a span of one of three buffers: the parsed
//! HTML, the raw-text tape, or the normalized-text tape. Which one depends
//! only on the field, so a whole column of values is a list of ranges of a
//! single buffer.

use std::ops::Range;

use super::columns::Columns;
use super::{ElementId, Store};

/// A string field of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field<'a> {
    /// The tag name, always saved.
    Name,
    /// [`ElementRef::inner_html`](crate::ElementRef::inner_html).
    InnerHtml,
    /// [`ElementRef::raw_text`](crate::ElementRef::raw_text).
    RawText,
    /// [`ElementRef::text`](crate::ElementRef::text).
    Text,
    /// [`ElementRef::class`](crate::ElementRef::class).
    Class,
    /// [`ElementRef::id`](crate::ElementRef::id).
    Id,
    /// [`ElementRef::attribute`](crate::ElementRef::attribute) of this name,
    /// which is never `class` or `id` when made by [`Field::attribute`].
    Attribute(&'a str),
}

/// A buffer that the values of a [`Field`] are byte ranges of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Buffer {
    /// The parsed HTML: names, inner HTML and attributes.
    Html,
    /// The raw-text tape.
    RawText,
    /// The normalized-text tape.
    Text,
}

impl<'a> Field<'a> {
    /// The field of the attribute `name`: [`Field::Class`] and [`Field::Id`]
    /// for those names, ASCII case-insensitively, like
    /// [`ElementRef::attribute`](crate::ElementRef::attribute) matches names.
    pub fn attribute(name: &'a str) -> Self {
        if name.eq_ignore_ascii_case("class") {
            Self::Class
        } else if name.eq_ignore_ascii_case("id") {
            Self::Id
        } else {
            Self::Attribute(name)
        }
    }

    /// The buffer this field's values are ranges of.
    #[inline]
    pub fn buffer(self) -> Buffer {
        match self {
            Self::RawText => Buffer::RawText,
            Self::Text => Buffer::Text,
            _ => Buffer::Html,
        }
    }
}

impl<'html, 'query: 'html> Store<'html, 'query> {
    /// The bytes of `buffer`. The HTML and text tapes are valid UTF-8 as a
    /// whole and at the ends of every range [`Store::field_range`] returns.
    #[inline]
    pub fn buffer(&self, buffer: Buffer) -> &[u8] {
        match buffer {
            Buffer::Html => self.html.as_bytes(),
            Buffer::RawText => self.text.raw_text.as_bytes(),
            Buffer::Text => self.text.text.as_bytes(),
        }
    }

    /// `row`'s value of `field`, as a range of the buffer
    /// [`Field::buffer`] names: the same string the matching
    /// [`ElementRef`](crate::ElementRef) getter returns, without slicing it.
    ///
    /// `None` where that getter returns `None`, or when `row` is not a row
    /// of this store.
    #[inline]
    pub fn field_range(&self, row: ElementId, field: Field<'_>) -> Option<Range<usize>> {
        let element = self.element(row)?;
        let index = row.index();
        let columns = &self.columns;
        let span = match field {
            Field::Name => columns.rows[index].name,
            Field::InnerHtml => return Columns::cell(&columns.inner_html, index),
            Field::RawText => return Columns::cell(&columns.raw_text, index),
            Field::Text => return Columns::cell(&columns.text, index),
            Field::Class => element.class_span(),
            Field::Id => element.id_span(),
            Field::Attribute(name) => element.attribute_span(name),
        };
        span.range()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Content, ElementRef, Section, StoreBuilder};
    use scah_html::{Attribute, XHtmlElement};

    static HTML: &str = "<a class=c id=i href=h title>x</a>";

    /// A store of one `a` row from `HTML`, saving everything.
    fn store() -> Store<'static, 'static> {
        let at = |text: &str| {
            let start = HTML.find(text).unwrap();
            &HTML[start..start + text.len()]
        };
        // Leaked so the row's attributes live as long as the HTML.
        let attributes: &'static [Attribute<'static>] = Box::leak(Box::new([
            Attribute {
                key: at("class"),
                value: Some(&HTML[9..10]),
            },
            Attribute {
                key: at("id"),
                value: Some(&HTML[14..15]),
            },
            Attribute {
                key: at("href"),
                value: Some(&HTML[21..22]),
            },
            Attribute {
                key: at("title"),
                value: None,
            },
        ]));
        let mut builder = StoreBuilder::default();
        builder.set_html(HTML);
        builder.set_sections(&[Section {
            selector: "a",
            parent: None,
            content: Content {
                inner_html: true,
                raw_text: true,
                text: true,
            },
        }]);
        let element = XHtmlElement {
            name: &HTML[1..2],
            class: Some(&HTML[9..10]),
            id: Some(&HTML[14..15]),
            attributes,
        };
        let row = builder.add_row(0, true, &element);
        builder.add_edge(None, row);
        builder.text_mut().raw_text.push_str("raw x");
        builder.text_mut().text.push_str("x");
        builder.set_content(row, Some(&HTML[29..30]), Some(4..5), Some(0..1));
        builder.finish()
    }

    /// `row`'s `field`, read back through its range.
    fn read<'s>(store: &'s Store, row: ElementId, field: Field) -> Option<&'s str> {
        let range = store.field_range(row, field)?;
        Some(std::str::from_utf8(&store.buffer(field.buffer())[range]).unwrap())
    }

    #[test]
    fn ranges_read_back_what_the_getters_return() {
        let store = store();
        let element: ElementRef = store.get("a").unwrap().next().unwrap();
        let row = element.index();
        let cases = [
            (Field::Name, Some(element.name())),
            (Field::InnerHtml, element.inner_html()),
            (Field::RawText, element.raw_text()),
            (Field::Text, element.text()),
            (Field::attribute("CLASS"), element.class()),
            (Field::attribute("id"), element.id()),
            (Field::attribute("href"), element.attribute("href")),
            (Field::attribute("title"), element.attribute("title")),
            (Field::attribute("missing"), None),
        ];
        for (field, expected) in cases {
            assert_eq!(read(&store, row, field), expected, "{field:?}");
            if let Some(expected) = expected {
                // The very bytes the getter borrows, not an equal copy.
                let value = read(&store, row, field).unwrap();
                assert_eq!(value.as_ptr(), expected.as_ptr(), "{field:?}");
            }
        }
        assert_eq!(element.attribute("title"), None, "valueless");
        assert_eq!(store.field_range(ElementId(1), Field::Name), None);
    }

    #[test]
    fn attribute_names_pick_the_dedicated_fields() {
        assert_eq!(Field::attribute("Class"), Field::Class);
        assert_eq!(Field::attribute("ID"), Field::Id);
        assert_eq!(Field::attribute("href"), Field::Attribute("href"));
        assert_eq!(Field::RawText.buffer(), Buffer::RawText);
        assert_eq!(Field::Attribute("x").buffer(), Buffer::Html);
    }
}
