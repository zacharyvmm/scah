use super::element::parser::XHtmlElement;
use crate::utils::reader::Reader;

enum XHtmlParser<'a> {
    Open(&'a str),  // <name>
    Close(&'a str), // </name>
}

impl<'a> XHtmlParser<'a> {
    fn next(reader: &mut Reader) {}
}

fn parse(reader: &mut Reader) {
    // move until it finds the first `<`
    reader.next_while(|c| c != '<');

    let parser = XHtmlElement::from(reader);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_html() {
        let mut reader = Reader::new(
            r#"
        <html>
            <h1>Hello World</h1>
            <p class="indent">
                My name is <span id="name" class="bold">Zachary</span>
            </p>
        </html>
        "#,
        );
    }
}
