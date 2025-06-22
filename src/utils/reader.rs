use std::iter::Peekable;
use std::ops::Range;
use std::str::Chars;

pub struct Reader<'a> {
    source: &'a str,
    iter: Peekable<Chars<'a>>,
    position: usize,
}

impl<'a> Reader<'a> {
    pub fn new(input: &'a str) -> Self {
        return Self {
            source: input,
            iter: input.chars().peekable(),
            position: 0,
        };
    }

    #[inline]
    pub fn get_position(&self) -> usize {
        return self.position;
    }

    #[inline]
    pub fn slice(&self, range: Range<usize>) -> &'a str {
        return &self.source[range];
    }

    #[inline]
    pub fn peek(&mut self) -> Option<char> {
        return match self.iter.peek() {
            Some(peek) => Some(*peek),
            _ => None,
        };
    }
}

impl<'a> Iterator for Reader<'a> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.position += 1;
        return self.iter.next();
    }
}

impl<'a> Reader<'a> {
    #[inline]
    pub fn next_while(&mut self, condition: fn(char) -> bool) {
        while let Some(character) = self.peek() {
            if condition(character) {
                self.next();
            } else {
                break;
            }
        }
    }

    // NOTE: next_while, but it consumes the character
    #[inline]
    pub fn next_upto(&mut self, condition: fn(char) -> bool) {
        while let Some(character) = self.next() {
            if !condition(character) {
                break;
            }
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
