#[macro_export]
macro_rules! mut_prt_unchecked {
    ($e:expr) => {{
        // Use a helper closure to enforce type preservation
        #[inline(always)]
        fn cast<T>(r: &T) -> *mut T {
            unsafe { r as *const T as *mut T }
        }
        cast($e)
    }}; //unsafe { &mut *($e as *const _ as *mut _) }
        // Usage: unsafe_const_ref_to_mut_ref(&value)
}
