// NOTE: This is a direct translation from the `api_design.py` file

use scrooge::{Condition /* ENUM */, HtmlParser, InnerContent, Selectors};

fn main() {
    let mut s = Selectors::new();
    s.add(
        "key",
        s.all("main > section").then(|section| {
            // Instead of Condition::Or I could do the same thing I do with python and implement the BitOr Trait
            // https://doc.rust-lang.org/core/ops/trait.BitOr.html
            // BUT: I don't think this kind of operator overloading is appreciated by rust users
            Condition::Or(
                section.first(
                    "p",
                    InnerContent {
                        inner_html: true,
                        text_content: false,
                    },
                ),
                section.all(
                    "a[href]",
                    InnerContent {
                        inner_html: false,
                        text_content: false,
                    },
                ),
            )
        }),
    );

    // For the same of efficiency I would like to be able to pass a immutable reference to the parser.
    // This would mean that the the Conditional FSM would handle the position
    let doc = HtmlParser::new("...", &s);

    doc.select("key");

    doc.selections();
}
