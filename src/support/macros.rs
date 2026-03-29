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
