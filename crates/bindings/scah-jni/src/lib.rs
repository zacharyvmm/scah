//! JNI backend of scah's Java binding.
//!
//! Implements the native methods of `io.github.zacharyvmm.scah.JniBackend`,
//! which the jar uses on Java versions without the Foreign Function and
//! Memory API. Like the Python and Node bindings, it calls [`scah_ffi`]
//! directly rather than the C ABI.
//!
//! Handles cross into Java as `long`s: boxed [`LazyQueryBuilder`]s,
//! [`OwnedQuery`]s, and [`OwnedStore`]s, freed by the matching `*Free`
//! method. The Java side keeps every handle alive for the duration of a call.
//! Failures throw `ScahException` with the status codes of `scah.h`; panics
//! never unwind into the JVM.

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::ReleaseMode;
use jni::objects::{
    JByteArray, JCharArray, JClass, JLongArray, JObject, JObjectArray, JString, JThrowable, JValue,
};
use jni::sys::{jboolean, jint, jlong};
use jni::{Env, EnvUnowned, jni_sig, jni_str};
use scah::lazy::{LazyQuery, LazyQueryBuilder};
use scah::{ElementId, ElementRef, ParseError, QuerySectionId, Save};
use scah_ffi::utf16::{LoneSurrogates, Utf16Encoder};
use scah_ffi::{OwnedQuery, OwnedStore};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Bumped with every incompatible change to the native methods, so a jar and
/// a library from different builds fail to load instead of misbehaving.
const PROTOCOL_VERSION: jint = 2;

// Status codes, as in scah.h.
const INVALID_UTF8: jint = 2;
const INVALID_SELECTOR: jint = 3;
const EMPTY_QUERIES: jint = 4;
const MAXIMUM_DEPTH_EXCEEDED: jint = 5;
const INDEX_OUT_OF_BOUNDS: jint = 6;
const INTERNAL_PANIC: jint = 7;
const INPUT_TOO_LARGE: jint = 8;

// `Save` flags, as in JniBackend.java.
const SAVE_INNER_HTML: jint = 1;
const SAVE_RAW_TEXT: jint = 2;
const SAVE_TEXT: jint = 4;
const SAVE_ATTRIBUTES: jint = 8;

// `Backend.Field` ordinals.
const FIELD_ID: jint = 0;
const FIELD_CLASS: jint = 1;
const FIELD_INNER_HTML: jint = 2;
const FIELD_RAW_TEXT: jint = 3;
const FIELD_TEXT: jint = 4;

/// Why a native method failed.
enum Failure {
    /// Thrown as `ScahException` with a status code.
    Scah(jint, String),
    /// A JNI call failed, usually with a Java exception already pending.
    Jni(jni::errors::Error),
}

impl From<jni::errors::Error> for Failure {
    fn from(error: jni::errors::Error) -> Self {
        Failure::Jni(error)
    }
}

impl From<ParseError> for Failure {
    fn from(error: ParseError) -> Self {
        let status = match error {
            ParseError::EmptyQueries => EMPTY_QUERIES,
            ParseError::MaximumDepthExceeded => MAXIMUM_DEPTH_EXCEEDED,
            ParseError::InputTooLarge => INPUT_TOO_LARGE,
            // Only `parse_without_text_capture` reports this.
            ParseError::TextCaptureRequired => INTERNAL_PANIC,
        };
        Failure::Scah(status, error.to_string())
    }
}

type Result<T> = std::result::Result<T, Failure>;

/// Run the body of a native method, turning failures and panics into Java
/// exceptions and returning `T::default()` with one pending.
fn call<'local, T: Default>(
    env: &mut EnvUnowned<'local>,
    body: impl FnOnce(&mut Env<'local>) -> Result<T>,
) -> T {
    env.with_env(|env| -> std::result::Result<T, jni::errors::Error> {
        let (status, message) = match catch_unwind(AssertUnwindSafe(|| body(env))) {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(Failure::Jni(error))) => return Err(error),
            Ok(Err(Failure::Scah(status, message))) => (status, message),
            Err(payload) => (INTERNAL_PANIC, panic_message(payload)),
        };
        // A pending exception, such as an OutOfMemoryError, takes precedence.
        if !env.exception_check() {
            throw(env, status, &message)?;
        }
        Ok(T::default())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Throw `ScahException(status, message)`.
fn throw(env: &mut Env<'_>, status: jint, message: &str) -> jni::errors::Result<()> {
    let message = env.new_string(message)?;
    let exception = env.new_object(
        jni_str!("io/github/zacharyvmm/scah/ScahException"),
        jni_sig!("(ILjava/lang/String;)V"),
        &[JValue::Int(status), JValue::Object(&message)],
    )?;
    let exception = env.cast_local::<JThrowable>(exception)?;
    env.throw(exception)
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    let detail = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("no message");
    format!("scah panicked: {detail}")
}

fn string(env: &Env<'_>, value: &JString<'_>) -> Result<String> {
    Ok(value.try_to_string(env)?)
}

fn save(flags: jint) -> Save {
    Save {
        inner_html: flags & SAVE_INNER_HTML != 0,
        raw_text: flags & SAVE_RAW_TEXT != 0,
        text: flags & SAVE_TEXT != 0,
        attributes: flags & SAVE_ATTRIBUTES != 0,
    }
}

fn into_handle<T>(value: T) -> jlong {
    Box::into_raw(Box::new(value)) as jlong
}

/// Borrow a handle the Java side keeps alive for the call.
///
/// # Safety
///
/// `handle` must come from [`into_handle`] with the same `T` and not have
/// been freed.
unsafe fn borrow<'a, T>(handle: jlong) -> &'a T {
    // SAFETY: guaranteed by the caller.
    unsafe { &*(handle as *const T) }
}

/// # Safety
///
/// Same as [`borrow`], and nothing else may use the handle meanwhile.
unsafe fn borrow_mut<'a, T>(handle: jlong) -> &'a mut T {
    // SAFETY: guaranteed by the caller.
    unsafe { &mut *(handle as *mut T) }
}

/// # Safety
///
/// `handle` must be 0, or come from [`into_handle`] with the same `T` and not
/// have been freed. It is freed by this call.
unsafe fn free<T>(handle: jlong) {
    if handle != 0 {
        // SAFETY: guaranteed by the caller.
        drop(unsafe { Box::from_raw(handle as *mut T) });
    }
}

/// Element `element` of the store behind `store`.
///
/// # Safety
///
/// `store` must be a live store handle.
unsafe fn element<'a>(store: jlong, element: jlong) -> Result<ElementRef<'a, 'a, 'a>> {
    // SAFETY: guaranteed by the caller.
    let store = unsafe { borrow::<OwnedStore>(store) }.store();
    usize::try_from(element)
        .ok()
        .and_then(|id| store.element(ElementId::from(id)))
        .ok_or_else(|| {
            Failure::Scah(
                INDEX_OUT_OF_BOUNDS,
                format!(
                    "element id {element} is out of bounds for a store of {} elements",
                    store.len()
                ),
            )
        })
}

fn ids<'local>(env: &mut Env<'local>, ids: Option<&[ElementId]>) -> Result<JLongArray<'local>> {
    let ids: Vec<jlong> = ids
        .unwrap_or_default()
        .iter()
        .map(|id| id.index() as jlong)
        .collect();
    let array = env.new_long_array(ids.len())?;
    array.set_region(env, 0, &ids)?;
    Ok(array)
}

fn optional<'local>(env: &mut Env<'local>, value: Option<&str>) -> Result<JString<'local>> {
    Ok(match value {
        Some(value) => env.new_string(value)?,
        None => JString::default(),
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_protocolVersion0<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jint {
    PROTOCOL_VERSION
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_builderNew0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    first: jboolean,
    selector: JString<'local>,
    flags: jint,
) -> jlong {
    call(&mut env, |env| {
        let selector = string(env, &selector)?;
        let builder = if first {
            LazyQuery::first(selector, save(flags))
        } else {
            LazyQuery::all(selector, save(flags))
        };
        Ok(into_handle(builder))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_builderAppend0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    builder: jlong,
    first: jboolean,
    selector: JString<'local>,
    flags: jint,
) {
    call(&mut env, |env| {
        let selector = string(env, &selector)?;
        // SAFETY: the Java side passes a live builder used by no other call.
        let builder = unsafe { borrow_mut::<LazyQueryBuilder<String>>(builder) };
        if first {
            builder.first_mut(selector, save(flags));
        } else {
            builder.all_mut(selector, save(flags));
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_builderThen0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    builder: jlong,
    children: JLongArray<'local>,
) {
    call(&mut env, |env| {
        let mut handles = vec![0; children.len(env)?];
        children.get_region(env, 0, &mut handles)?;
        // SAFETY: the Java side passes a live builder used by no other call.
        let builder = unsafe { borrow_mut::<LazyQueryBuilder<String>>(builder) };
        let last = QuerySectionId(builder.len() - 1);
        for child in handles {
            // SAFETY: the Java side passes live builders.
            let child = unsafe { borrow::<LazyQueryBuilder<String>>(child) };
            builder.append(last, child.clone());
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_builderFree0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    builder: jlong,
) {
    call(&mut env, |_| {
        // SAFETY: the Java side frees each builder once.
        unsafe { free::<LazyQueryBuilder<String>>(builder) };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_build0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    builder: jlong,
) -> jlong {
    call(&mut env, |_| {
        // SAFETY: the Java side passes a live builder.
        let builder = unsafe { borrow::<LazyQueryBuilder<String>>(builder) };
        let query = OwnedQuery::build(builder.clone())
            .map_err(|error| Failure::Scah(INVALID_SELECTOR, error.to_string()))?;
        Ok(into_handle(query))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_queryFree0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    query: jlong,
) {
    call(&mut env, |_| {
        // SAFETY: the Java side frees each query once, when no call uses it.
        unsafe { free::<OwnedQuery>(query) };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_parse0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    html: JByteArray<'local>,
    queries: JLongArray<'local>,
) -> jlong {
    call(&mut env, |env| {
        let mut handles = vec![0; queries.len(env)?];
        queries.get_region(env, 0, &mut handles)?;
        let html = String::from_utf8(env.convert_byte_array(&html)?).map_err(|error| {
            Failure::Scah(INVALID_UTF8, format!("`html` is not valid UTF-8: {error}"))
        })?;
        // SAFETY: the Java side keeps every query alive for the call.
        let queries: Vec<&OwnedQuery> = handles
            .iter()
            .map(|&query| unsafe { borrow(query) })
            .collect();
        Ok(into_handle(OwnedStore::parse(html, &queries)?))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_htmlNew0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    capacity: jlong,
) -> jlong {
    call(&mut env, |_| {
        // The parser rejects 4 GiB or more, so reserving more is pointless.
        let capacity = (capacity.max(0) as u64).min(u64::from(u32::MAX)) as usize;
        Ok(into_handle(Utf16Encoder::with_capacity(
            capacity,
            LoneSurrogates::Replace,
        )))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_htmlPush0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    html: jlong,
    units: JCharArray<'local>,
    length: jint,
) {
    call(&mut env, |env| {
        // SAFETY: the Java side owns the HTML and uses it from one thread.
        let html = unsafe { borrow_mut::<Utf16Encoder>(html) };
        let length = length.max(0) as usize;
        if html.len().saturating_add(length) >= u32::MAX as usize {
            return Err(Failure::Scah(
                INPUT_TOO_LARGE,
                "the HTML is 4 GiB or longer".into(),
            ));
        }
        // Grow the buffer before pinning: a reallocation copies the whole
        // document so far. Encoding is then bounded by `length`, which the
        // Java side keeps to one block, so the pin is short.
        html.reserve(length);
        // SAFETY: no JNI calls are made while the elements are held.
        let elements = unsafe { units.get_elements_critical(env, ReleaseMode::NoCopyBack) }?;
        let length = length.min(elements.len());
        html.push(&elements[..length])
            .expect("replacing lone surrogates cannot fail");
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_htmlFree0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    html: jlong,
) {
    call(&mut env, |_| {
        // SAFETY: the Java side frees unparsed HTML once.
        unsafe { free::<Utf16Encoder>(html) };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_parseHtml0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    html: jlong,
    queries: JLongArray<'local>,
) -> jlong {
    // Take ownership first, so the HTML is freed on every path.
    // SAFETY: the Java side hands over HTML it never uses again.
    let html = unsafe { Box::from_raw(html as *mut Utf16Encoder) };
    call(&mut env, |env| {
        let mut handles = vec![0; queries.len(env)?];
        queries.get_region(env, 0, &mut handles)?;
        let html = html
            .finish()
            .expect("replacing lone surrogates cannot fail");
        // SAFETY: the Java side keeps every query alive for the call.
        let queries: Vec<&OwnedQuery> = handles
            .iter()
            .map(|&query| unsafe { borrow(query) })
            .collect();
        Ok(into_handle(OwnedStore::parse(html, &queries)?))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_storeFree0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
) {
    call(&mut env, |_| {
        // SAFETY: the Java side frees each store once, when no call uses it.
        unsafe { free::<OwnedStore>(store) };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_storeLength0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
) -> jlong {
    call(&mut env, |_| {
        // SAFETY: the Java side keeps the store alive for the call.
        Ok(unsafe { borrow::<OwnedStore>(store) }.store().len() as jlong)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_storeGet0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
    selector: JString<'local>,
) -> JLongArray<'local> {
    call(&mut env, |env| {
        let selector = string(env, &selector)?;
        // SAFETY: the Java side keeps the store alive for the call.
        let store = unsafe { borrow::<OwnedStore>(store) }.store();
        ids(env, store.results(&selector))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_storeQuery0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
    index: jlong,
) -> JLongArray<'local> {
    call(&mut env, |env| {
        // SAFETY: the Java side keeps the store alive for the call.
        let store = unsafe { borrow::<OwnedStore>(store) }.store();
        let results = usize::try_from(index)
            .ok()
            .and_then(|index| store.query_results(index));
        ids(env, results)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_elementGet0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
    id: jlong,
    selector: JString<'local>,
) -> JLongArray<'local> {
    call(&mut env, |env| {
        let selector = string(env, &selector)?;
        // SAFETY: the Java side keeps the store alive for the call.
        let parent = unsafe { element(store, id) }?;
        // SAFETY: as above.
        let store = unsafe { borrow::<OwnedStore>(store) }.store();
        ids(env, store.child_results(parent.index(), &selector))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_name0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
    id: jlong,
) -> JString<'local> {
    call(&mut env, |env| {
        // SAFETY: the Java side keeps the store alive for the call.
        let element = unsafe { element(store, id) }?;
        Ok(env.new_string(element.name())?)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_field0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
    id: jlong,
    field: jint,
) -> JString<'local> {
    call(&mut env, |env| {
        // SAFETY: the Java side keeps the store alive for the call.
        let element = unsafe { element(store, id) }?;
        let value = match field {
            FIELD_ID => element.id(),
            FIELD_CLASS => element.class(),
            FIELD_INNER_HTML => element.inner_html(),
            FIELD_RAW_TEXT => element.raw_text(),
            FIELD_TEXT => element.text(),
            _ => unreachable!("unknown field {field}"),
        };
        optional(env, value)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_attribute0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
    id: jlong,
    key: JString<'local>,
) -> JString<'local> {
    call(&mut env, |env| {
        let key = string(env, &key)?;
        // SAFETY: the Java side keeps the store alive for the call.
        let element = unsafe { element(store, id) }?;
        optional(env, element.attribute(&key))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_zacharyvmm_scah_JniBackend_attributes0<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    store: jlong,
    id: jlong,
) -> JObjectArray<'local> {
    call(&mut env, |env| {
        // SAFETY: the Java side keeps the store alive for the call.
        let element = unsafe { element(store, id) }?;
        let attributes: Vec<_> = element.attributes().into_iter().flatten().collect();
        let pairs = env.new_object_array(
            (attributes.len() * 2) as jint,
            jni_str!("java/lang/String"),
            JObject::null(),
        )?;
        for (index, attribute) in attributes.iter().enumerate() {
            let key = env.new_string(attribute.key)?;
            pairs.set_element(env, 2 * index, &key)?;
            if let Some(value) = attribute.value {
                let value = env.new_string(value)?;
                pairs.set_element(env, 2 * index + 1, &value)?;
            }
        }
        Ok(pairs)
    })
}
