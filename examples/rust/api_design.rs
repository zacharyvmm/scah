// NOTE: This is a direct translation from the `api_design.py` file

use scrooge::{HtmlParser, InnerContent, Selectors, Reader};

fn main() {
    let mut s = Selectors::new();
    s.add(
        "key",
        s.all("main > section").first("div").or(
            Selector::new().first(
                Reader::new("p"),
                InnerContent {
                    inner_html: true,
                    text_content: false,
                },
            ),
            Selector::new().all(
                Reader::new("a[href]"),
                InnerContent {
                    inner_html: false,
                    text_content: false,
                },
            ),
        )
    );

    // For the same of efficiency I would like to be able to pass a immutable reference to the parser.
    // This would mean that the the Conditional FSM would handle the position
    let doc = HtmlParser::new("...", &s);

    doc.select("key");

    doc.selections();
}
