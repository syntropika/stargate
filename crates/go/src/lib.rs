//! Versioned C ABI. Handles are registry IDs, so stale handles produce errors instead of dereferencing freed memory.
use rust::NativeRuntime;
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
static RUNTIMES: OnceLock<Mutex<HashMap<u64, Arc<NativeRuntime>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn runtimes() -> &'static Mutex<HashMap<u64, Arc<NativeRuntime>>> {
    RUNTIMES.get_or_init(Mutex::default)
}
#[unsafe(no_mangle)]
pub extern "C" fn stargate_abi_version() -> u32 {
    1
}
/// Call create (handle=0), handle or authorize. The returned UTF-8 JSON must be freed with stargate_free.
/// # Safety
/// operation and input must point to valid NUL-terminated strings for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stargate_call(
    handle: u64,
    operation: *const c_char,
    input: *const c_char,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> std::result::Result<String, String> {
        if operation.is_null() || input.is_null() {
            return Err("invalid request".into());
        }
        let operation = unsafe { CStr::from_ptr(operation) }
            .to_str()
            .map_err(|_| "invalid request")?;
        let input = unsafe { CStr::from_ptr(input) }
            .to_str()
            .map_err(|_| "invalid request")?;
        if operation == "create" && handle == 0 {
            let runtime = Arc::new(NativeRuntime::create(input)?);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            runtimes()
                .lock()
                .map_err(|_| "runtime unavailable")?
                .insert(id, runtime);
            return Ok(serde_json::json!({"handle":id}).to_string());
        }
        let runtime = runtimes()
            .lock()
            .map_err(|_| "runtime unavailable")?
            .get(&handle)
            .cloned()
            .ok_or("invalid handle")?;
        match operation {
            "handle" => runtime.handle(input),
            "authorize" => runtime.authorize(input),
            _ => Err("invalid operation".into()),
        }
    })
    .unwrap_or_else(|_| Err("runtime unavailable".into()));
    let json = result.unwrap_or_else(|error| serde_json::json!({"error":error}).to_string());
    CString::new(json)
        .expect("JSON contains no NUL bytes")
        .into_raw()
}
#[unsafe(no_mangle)]
pub extern "C" fn stargate_destroy(handle: u64) -> u32 {
    match runtimes().lock() {
        Ok(mut r) => {
            if r.remove(&handle).is_some() {
                0
            } else {
                1
            }
        }
        Err(_) => 2,
    }
}
/// # Safety
/// ptr must be a still-owned pointer returned by stargate_call, freed exactly once, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stargate_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}
