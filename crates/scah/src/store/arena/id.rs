pub(crate) use super::iter::Nullable;

const NULL: usize = usize::MAX;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
        pub struct $name(pub(crate) usize);

        impl $name {
            #[inline(always)]
            pub fn index(&self) -> usize {
                self.0
            }
        }

        impl Nullable for $name {
            #[inline(always)]
            fn is_null(&self) -> bool {
                self.0 == NULL
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self(NULL)
            }
        }

        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(value)
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}
define_id!(ElementId);
define_id!(QueryId);
define_id!(AttributeId);
