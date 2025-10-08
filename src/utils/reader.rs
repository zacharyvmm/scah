use std::iter::Peekable;
use std::ops::Range;
use std::str::CharIndices;

pub struct Reader<'a> {
    source: &'a str,
    iter: Peekable<CharIndices<'a>>,
    position: usize,
}

impl<'a> Reader<'a> {
    pub fn new(input: &'a str) -> Self {
        return Self {
            source: input,
            iter: input.char_indices().peekable(),
            position: 0,
        };
    }

    #[inline]
    pub fn get_position(&self) -> usize {
        // dynamicly determine the position
        // self.position = self.iter.peek().map(|(p, _)| *p).unwrap_or(self.source.len());
        return self.position;
    }

    #[inline]
    pub fn slice(&self, range: Range<usize>) -> &'a str {
        return &self.source[range];
    }

    #[inline]
    pub fn peek(&mut self) -> Option<char> {
        return match self.iter.peek() {
            Some((_, peek)) => {
                Some(*peek)
            },
            None => None,
        };
    }

    #[inline]
    pub fn next_while(&mut self, condition: fn(char) -> bool) {
        while let Some(_) = self.iter.next_if(|(_, c)| condition(*c)) {}
        self.position = self.iter.peek().map(|(p, _)| *p).unwrap_or(self.source.len());
    }

    pub fn skip(&mut self) {
        _ = self.iter.next();
        self.position = self.iter.peek().map(|(p, _)| *p).unwrap_or(self.source.len());
    }
}

impl<'a> Iterator for Reader<'a> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some((_, c)) => {
                self.position = self.iter.peek().map(|(p, _)| *p).unwrap_or(self.source.len());
                Some(c)
            }
            None => None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_iterator() {
        let my_string = String::from("Hello World");
        let mut reader = Reader::new(&my_string);

        assert_eq!(reader.get_position(), 0);
        assert_eq!(reader.next(), Some('H'));

        assert_eq!(reader.get_position(), 1);
        assert_eq!(reader.next(), Some('e'));

        assert_eq!(reader.get_position(), 2);
        assert_eq!(reader.next(), Some('l'));

        assert_eq!(reader.get_position(), 3);
        assert_eq!(reader.next(), Some('l'));

        assert_eq!(reader.get_position(), 4);
        assert_eq!(reader.next(), Some('o'));

        assert_eq!(reader.get_position(), 5);
        assert_eq!(reader.next(), Some(' '));

        assert_eq!(reader.get_position(), 6);
        assert_eq!(reader.next(), Some('W'));

        assert_eq!(reader.get_position(), 7);
        assert_eq!(reader.next(), Some('o'));

        assert_eq!(reader.get_position(), 8);
        assert_eq!(reader.next(), Some('r'));

        assert_eq!(reader.get_position(), 9);
        assert_eq!(reader.next(), Some('l'));

        assert_eq!(reader.get_position(), 10);
        assert_eq!(reader.peek(), Some('d'));

        assert_eq!(reader.get_position(), 10);
        assert_eq!(reader.next(), Some('d'));

        assert_eq!(reader.slice(0..5), "Hello");
    }
}
