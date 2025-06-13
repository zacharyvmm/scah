#[derive(Debug, PartialEq)]
pub enum QuoteKind {
    DoubleQuoted,
    SingleQuoted,
}

#[derive(Debug, PartialEq)]
pub enum KeyValueToken<'a> {
    String(&'a str),
    Quote(QuoteKind),
    Equal,
}
