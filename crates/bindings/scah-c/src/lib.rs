//! C ABI for [`scah`].
//!
//! The library builds as `libscah_c.a` and `libscah_c.so` /
//! `libscah_c.dylib`, and `include/scah.h` declares every function exported
//! here.
//!
//! # Workflow
//!
//! 1. Describe a query with a [`ScahQueryBuilder`], nesting child builders
//!    with [`scah_query_builder_then`], and compile it with [`scah_query_build`].
//! 2. Parse HTML against one or more queries with [`scah_parse`].
//! 3. Look up matches with [`scah_store_get`] and [`scah_element_get`], then
//!    read element fields with the `scah_element_*` getters.
//!
//! # Ownership
//!
//! - Every handle returned through an out-pointer is owned by the caller and
//!   released with its `*_free` function. Every `*_free` function accepts NULL.
//! - A store copies its HTML and shares its queries' selector strings, so the
//!   HTML buffer, queries, and builders may be freed as soon as
//!   [`scah_parse`] returns.
//! - Element lists are independent of their store, but their ids are only
//!   meaningful together with the store that produced them.
//! - String views returned by getters borrow from the store and stay valid
//!   until [`scah_store_free`]. The view returned by [`scah_error_message`]
//!   stays valid until [`scah_error_free`].
//! - String views are UTF-8 and are not NUL-terminated.
//!
//! # Errors
//!
//! Fallible functions return a [`ScahStatus`] and take a trailing
//! `ScahError **out_error`, which may be NULL. On entry the function sets
//! `*out_error` to NULL. On failure it stores a newly allocated error there
//! for the caller to free. Out-pointers for handles are likewise set to NULL
//! before any work happens. Panics never unwind into C: they are reported as
//! [`ScahStatus::InternalPanic`].
//!
//! # Threads
//!
//! Queries and stores are immutable and may be shared across threads.
//! Builders, element lists, and errors must not be used from two threads at
//! once.

mod error;
mod query;
mod store;
mod string;

pub use error::{ScahError, ScahStatus, scah_error_free, scah_error_message};
pub use query::{
    ScahQuery, ScahQueryBuilder, ScahSave, scah_query_build, scah_query_builder_all,
    scah_query_builder_clone, scah_query_builder_first, scah_query_builder_free,
    scah_query_builder_new_all, scah_query_builder_new_first, scah_query_builder_then,
    scah_query_free,
};
pub use store::{
    ScahElementId, ScahElementList, ScahStore, scah_element_attribute, scah_element_attribute_at,
    scah_element_attribute_count, scah_element_class_name, scah_element_get, scah_element_id,
    scah_element_inner_html, scah_element_list_free, scah_element_list_ids, scah_element_list_len,
    scah_element_name, scah_element_raw_text, scah_element_text, scah_parse, scah_store_free,
    scah_store_get, scah_store_len,
};
pub use string::{ScahOptionalStringView, ScahStringView};

/// Version of the C ABI described by `scah.h`.
///
/// Bumped on every incompatible change to a signature, type layout, or
/// documented behavior.
pub const SCAH_ABI_VERSION: u32 = 1;

/// The ABI version of the loaded library.
///
/// Compare against `SCAH_ABI_VERSION` to detect a header and library mismatch.
#[unsafe(no_mangle)]
pub extern "C" fn scah_abi_version() -> u32 {
    SCAH_ABI_VERSION
}
