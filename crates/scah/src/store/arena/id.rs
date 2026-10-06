const NULL: u32 = u32::MAX;

macro_rules! define_id {
    ($name:ident) => {
        /// A row index. Stores hold at most `u32::MAX` rows.
        #[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
        pub struct $name(pub(crate) u32);

        impl $name {
            #[inline(always)]
            pub fn index(&self) -> usize {
                self.0 as usize
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self(NULL)
            }
        }

        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(u32::try_from(value).expect("store rows fit in u32"))
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.0 as usize
            }
        }
    };
}
define_id!(ElementId);
define_id!(AttributeId);
