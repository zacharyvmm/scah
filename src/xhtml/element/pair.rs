use super::parser::Attribute;

pub enum Pair<'a> {
    NewKey,
    Equal(&'a str),
    AssignValue(&'a str), // the string is the key buffer
}

impl<'a> Pair<'a> {
    pub fn set_assign_value(&mut self) {
        if let Self::Equal(key) = self {
            *self = Self::AssignValue(key);
        }
    }

    // If the string ends with a `key` without a value then this would get the final value
    pub fn get_final_equal_value(&mut self) -> Option<Attribute<'a>> {
        if let Self::Equal(key) = self {
            return Some(Attribute {
                name: *key,
                value: None,
            });
        }

        return None;
    }

    pub fn add_string(&mut self, content: &'a str) -> Option<Attribute<'a>> {
        match self {
            Self::NewKey => {
                *self = Self::Equal(content);
                return None;
            }

            Self::Equal(key) => {
                let ret = Some(Attribute {
                    name: *key,
                    value: None,
                });

                *self = Self::Equal(content);

                return ret;
            }

            Self::AssignValue(key) => {
                let ret = Some(Attribute {
                    name: *key,
                    value: Some(content),
                });

                *self = Self::NewKey;

                return ret;
            }
        }
    }
}
