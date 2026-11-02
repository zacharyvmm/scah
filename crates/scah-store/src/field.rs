//! Rows read as byte ranges, for consumers that address values as offsets
//! into the store's buffers instead of copying them, such as scah-ffi's
//! Arrow export.
//!
//! Every string a store holds is a span of one of four buffers: the parsed
//! HTML, the raw-text tape, the normalized-text tape, or the tape of decoded
//! attribute values. Which one depends only on the field, except for
//! attribute values: those with `&`, CR, or NUL are in the decoded tape and
//! the others in the HTML. So a column of values is a list of
//! ranges of at most two buffers.

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
    /// The parsed HTML: names, inner HTML, and attribute values without
    /// `&`, CR, or NUL.
    Html,
    /// The raw-text tape.
    RawText,
    /// The normalized-text tape.
    Text,
    /// Attribute values that had `&`, CR, or NUL, decoded.
    Attributes,
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
}

impl<'html, 'query: 'html> Store<'html, 'query> {
    /// The bytes of `buffer`. Each buffer is valid UTF-8 as a whole and at
    /// the ends of every range [`Store::field_range`] returns.
    /// [`Buffer::Attributes`] decodes the store's attribute values with
    /// `&`, CR, or NUL, if no read has yet.
    #[inline]
    pub fn buffer(&self, buffer: Buffer) -> &[u8] {
        match buffer {
            Buffer::Html => self.html.as_bytes(),
            Buffer::RawText => self.text.raw_text.as_bytes(),
            Buffer::Text => self.text.text.as_bytes(),
            Buffer::Attributes => self.decoded().text.as_bytes(),
        }
    }

    /// `row`'s value of `field`, as a buffer and a range of it: the same
    /// string the matching [`ElementRef`](crate::ElementRef) getter returns,
    /// without slicing it.
    ///
    /// The buffer is [`Buffer::Html`] for the name, inner HTML, and
    /// attribute values without `&`, CR, or NUL,
    /// [`Buffer::Attributes`] for attribute values with them, and the
    /// matching tape for the text fields.
    ///
    /// `None` where that getter returns `None`, or when `row` is not a row
    /// of this store.
    #[inline]
    pub fn field_range(&self, row: ElementId, field: Field<'_>) -> Option<(Buffer, Range<usize>)> {
        let element = self.element(row)?;
        let index = row.index();
        let columns = &self.columns;
        let attribute = match field {
            Field::Name => return Some((Buffer::Html, columns.rows[index].name.range()?)),
            Field::InnerHtml => {
                return Columns::cell(&columns.inner_html, index)
                    .map(|range| (Buffer::Html, range));
            }
            Field::RawText => {
                return Columns::cell(&columns.raw_text, index)
                    .map(|range| (Buffer::RawText, range));
            }
            Field::Text => {
                return Columns::cell(&columns.text, index).map(|range| (Buffer::Text, range));
            }
            Field::Class => element.class_index(),
            Field::Id => element.id_index(),
            Field::Attribute(name) => element.attribute_index(name),
        };
        self.attribute_location(attribute?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Content, ElementRef, Section, StoreBuilder};
    use scah_html::{Attribute, XHtmlElement};

    static HTML: &str = "<a class=c&amp;d id=i href=h&lt; rel=r dir='l\r\nr' title>x</a>";

    /// The first `text` in `HTML`.
    fn at(text: &str) -> &'static str {
        let start = HTML.find(text).unwrap();
        &HTML[start..start + text.len()]
    }

    /// A store of one `a` row from `HTML`, saving everything.
    fn store() -> Store<'static, 'static> {
        // Leaked so the row's attributes live as long as the HTML.
        let attributes: &'static [Attribute<'static>] = Box::leak(Box::new([
            Attribute {
                key: at("class"),
                value: Some(at("c&amp;d")),
            },
            Attribute {
                key: at("id"),
                value: Some(&at("i ")[..1]),
            },
            Attribute {
                key: at("href"),
                value: Some(at("h&lt;")),
            },
            Attribute {
                key: at("rel"),
                value: Some(&at("r ")[..1]),
            },
            Attribute {
                key: at("dir"),
                value: Some(at("l\r\nr")),
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
            class: attributes[0].value,
            id: attributes[1].value,
            attributes,
        };
        let row = builder.add_row(0, true, &element);
        builder.add_edge(None, row);
        builder.text_mut().raw_text.push_str("raw x");
        builder.text_mut().text.push_str("x");
        builder.set_content(row, Some(&at("x<")[..1]), Some(4..5), Some(0..1));
        builder.finish()
    }

    /// `row`'s `field`, read back through its range, and the buffer it is in.
    fn read<'s>(store: &'s Store, row: ElementId, field: Field) -> Option<(Buffer, &'s str)> {
        let (buffer, range) = store.field_range(row, field)?;
        let value = std::str::from_utf8(&store.buffer(buffer)[range]).unwrap();
        Some((buffer, value))
    }

    #[test]
    fn ranges_read_back_what_the_getters_return() {
        let store = store();
        let element: ElementRef = store.get("a").unwrap().next().unwrap();
        let row = element.index();
        let cases = [
            (Field::Name, Buffer::Html, Some(element.name())),
            (Field::InnerHtml, Buffer::Html, element.inner_html()),
            (Field::RawText, Buffer::RawText, element.raw_text()),
            (Field::Text, Buffer::Text, element.text()),
            (
                Field::attribute("CLASS"),
                Buffer::Attributes,
                element.class(),
            ),
            (Field::attribute("id"), Buffer::Html, element.id()),
            (
                Field::attribute("href"),
                Buffer::Attributes,
                element.attribute("href"),
            ),
            (
                Field::attribute("rel"),
                Buffer::Html,
                element.attribute("rel"),
            ),
            (
                Field::attribute("dir"),
                Buffer::Attributes,
                element.attribute("dir"),
            ),
            (
                Field::attribute("title"),
                Buffer::Html,
                element.attribute("title"),
            ),
            (Field::attribute("missing"), Buffer::Html, None),
        ];
        for (field, buffer, expected) in cases {
            let value = read(&store, row, field);
            assert_eq!(value.map(|(_, value)| value), expected, "{field:?}");
            if let (Some((found, value)), Some(expected)) = (value, expected) {
                assert_eq!(found, buffer, "{field:?}");
                // The very bytes the getter borrows, not an equal copy.
                assert_eq!(value.as_ptr(), expected.as_ptr(), "{field:?}");
            }
        }
        assert_eq!(element.class(), Some("c&d"));
        assert_eq!(element.attribute("href"), Some("h<"));
        assert_eq!(element.attribute("dir"), Some("l\nr"));
        assert_eq!(element.attribute("title"), None, "valueless");
        assert_eq!(store.field_range(ElementId(1), Field::Name), None);
    }

    #[test]
    fn attribute_names_pick_the_dedicated_fields() {
        assert_eq!(Field::attribute("Class"), Field::Class);
        assert_eq!(Field::attribute("ID"), Field::Id);
        assert_eq!(Field::attribute("href"), Field::Attribute("href"));
    }
}
