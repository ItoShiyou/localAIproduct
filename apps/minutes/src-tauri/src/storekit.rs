//! Native StoreKit is the sole source of MAS rights. No frontend-supplied Pro flag.
use std::sync::atomic::{AtomicBool, Ordering};
static PRO: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
pub fn is_ready() -> bool { READY.load(Ordering::Acquire) }
pub fn is_pro() -> bool { PRO.load(Ordering::Acquire) }
#[cfg(all(feature = "mac-app-store", target_os = "macos"))]
mod native {
    use super::*;
    use std::ffi::{c_char, c_void, CStr};
    type Callback = extern "C" fn(*const c_char, *mut c_void);
    extern "C" {
        fn minutes_storekit_start(callback: Callback);
        fn minutes_storekit_request(action: i32, callback: Callback, context: *mut c_void);
    }
    fn parse(ptr: *const c_char) -> Option<serde_json::Value> {
        if ptr.is_null() { return None; }
        serde_json::from_slice(unsafe { CStr::from_ptr(ptr) }.to_bytes()).ok()
    }
    extern "C" fn update(ptr: *const c_char, _: *mut c_void) {
        if let Some(value) = parse(ptr) { PRO.store(value["pro"].as_bool().unwrap_or(false), Ordering::Release);
            READY.store(true, Ordering::Release); }
    }
    extern "C" fn response(ptr: *const c_char, ctx: *mut c_void) {
        let sender = unsafe { Box::from_raw(ctx as *mut std::sync::mpsc::Sender<serde_json::Value>) };
        if let Some(value) = parse(ptr) {
            PRO.store(value["pro"].as_bool().unwrap_or(false), Ordering::Release);
            READY.store(true, Ordering::Release);
            let _ = sender.send(value);
        }
    }
    pub fn start() { unsafe { minutes_storekit_start(update); } }
    pub fn request(action: i32) -> Result<serde_json::Value, String> {
        let (sender, receiver) = std::sync::mpsc::channel::<serde_json::Value>();
        unsafe { minutes_storekit_request(action, response, Box::into_raw(Box::new(sender)) as *mut c_void); }
        receiver.recv().map_err(|_| "購入状態を確認できませんでした".into())
    }
}
#[cfg(all(feature = "mac-app-store", target_os = "macos"))]
pub use native::{start, request};
