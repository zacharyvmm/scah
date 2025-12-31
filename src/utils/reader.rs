use std::ops::Range;

pub struct Reader<'a> {
    source: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub fn new(input: &'a str) -> Self {
        return Self {
            source: input.as_bytes(),
            position: 0,
        };
    }

    #[inline]
    pub fn get_position(&self) -> usize {
        return self.position;
    }

    #[inline]
    pub fn slice(&self, range: Range<usize>) -> &'a str {
        // SAFETY: The source was originally a &str, and structural characters are ASCII.
        // We should be careful about slicing in the middle of a UTF-8 character.
        return unsafe { std::str::from_utf8_unchecked(&self.source[range]) };
    }

    #[inline]
    pub fn peek(&self) -> Option<u8> {
        return self.source.get(self.position).copied();
    }

    // NOTE: Escaping characters has a significant cost
    // #[inline]
    // pub fn next_while<F>(&mut self, condition: F) 
    // where F: Fn(u8) -> bool {
    //     while self.position < self.source.len() {
    //         let b = self.source[self.position];
    //         let should_continue = if last_escape_character {
    //             last_escape_character = false;
    //             true
    //         } else if b == ESCAPE_CHARACTER {
    //             last_escape_character = true;
    //             true
    //         } else {
    //             condition(b)
    //         };

    //         if should_continue {
    //             self.position += 1;
    //         } else {
    //             break;
    //         }
    //     }
    // }

    #[inline]
    pub fn next_while<F>(&mut self, condition: F) 
    where F: Fn(u8) -> bool {
        while self.position < self.source.len() {
            let b = self.source[self.position];
            let should_continue = condition(b);

            if should_continue {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    pub fn skip(&mut self) {
        if self.position < self.source.len() {
            self.position += 1;
        }
    }

    pub fn eof(&self) -> bool {
        if self.position >= self.source.len() {
            return true;
        }

        if self.source[self.position..]
            .iter()
            .all(|b| b.is_ascii_whitespace())
        {
            return true;
        }

        return false;
    }
}

impl<'a> Iterator for Reader<'a> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.position < self.source.len() {
            let b = self.source[self.position];
            self.position += 1;
            Some(b)
        } else {
            None
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
        assert_eq!(reader.next(), Some(b'H'));

        assert_eq!(reader.get_position(), 1);
        assert_eq!(reader.next(), Some(b'e'));

        assert_eq!(reader.get_position(), 2);
        assert_eq!(reader.next(), Some(b'l'));

        assert_eq!(reader.get_position(), 3);
        assert_eq!(reader.next(), Some(b'l'));

        assert_eq!(reader.get_position(), 4);
        assert_eq!(reader.next(), Some(b'o'));

        assert_eq!(reader.get_position(), 5);
        assert_eq!(reader.next(), Some(b' '));

        assert_eq!(reader.get_position(), 6);
        assert_eq!(reader.next(), Some(b'W'));

        assert_eq!(reader.get_position(), 7);
        assert_eq!(reader.next(), Some(b'o'));

        assert_eq!(reader.get_position(), 8);
        assert_eq!(reader.next(), Some(b'r'));

        assert_eq!(reader.get_position(), 9);
        assert_eq!(reader.next(), Some(b'l'));

        assert_eq!(reader.get_position(), 10);
        assert_eq!(reader.peek(), Some(b'd'));

        assert_eq!(reader.get_position(), 10);
        assert_eq!(reader.next(), Some(b'd'));

        assert_eq!(reader.slice(0..5), "Hello");
    }
}