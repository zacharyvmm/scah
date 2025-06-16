fn parse(reader: &mut Reader) {
    // move until it finds the first `<`
    reader.next_while(|c| c != '<');

    let mut parser = AttributeParser::new(reader);
    parser.parse();

    assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_html() {
        let reader = Reader::new(
            "
        <html>
            <h1>Hello World</h1>
            <p class=\"indent\">
                My name is <span id=\"name\" class=\"bold\">Zachary</span>
            </p>
        </html>
        ",
        );
    }
}
