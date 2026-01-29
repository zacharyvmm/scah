#[macro_export]
macro_rules! mut_prt_unchecked {
    ($e:expr) => {{
        #[inline(always)]
        fn cast<T>(r: &T) -> *mut T {
            r as *const T as *mut T
        }
        cast($e)
    }};
}

#[macro_export]
macro_rules! dbg_print {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            println!($($arg)*);
        }
    }
}

#[macro_export]
#[deprecated(note = "To be removed")]
macro_rules! to_str {
    ($e:expr) => {{
        #[inline(always)]
        fn to_str(bytes: &[u8]) -> &str {
            unsafe { str::from_utf8_unchecked(bytes) }
        }
        to_str($e)
    }};
}