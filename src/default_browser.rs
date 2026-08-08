//! Default web browser: read and set the LaunchServices `http` handler.
//!
//! macOS has gated the default-browser change behind its own confirmation
//! panel since 10.13, and 26.4 extended that panel to every remaining file
//! type (<https://scriptingosx.com/2026/03/macos-26-4-brings-more-default-app-confirmation-prompts/>).
//! Editing `com.apple.launchservices.secure.plist` is not a way around it:
//! LaunchServices reverts a handler change it did not ask for, and the
//! plist-editing workarounds mac admins still use only hold when they run
//! before the user's LaunchServices daemon starts. macOS also has no
//! configuration profile for the default browser. So the supported call is the
//! one below, the panel it raises is the sanctioned human surface, and the job
//! here is to raise it, put it in front, and poll for the answer.

use crate::error::{ActionError, ActionResult};
use crate::headless;
use std::time::{Duration, Instant};

/// Aside (aside.com). Homebrew cask `aside`.
pub const ASIDE_BUNDLE_ID: &str = "at.studio.AsideBrowser";

/// Bundle id currently registered for `http`.
pub fn current_handler() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos::handler_for_scheme("http")
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub fn is_default(bundle_id: &str) -> bool {
    current_handler()
        .map(|id| id.eq_ignore_ascii_case(bundle_id))
        .unwrap_or(false)
}

pub fn is_installed(bundle_id: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::app_path(bundle_id).is_some()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle_id;
        false
    }
}

/// Ask macOS to make `bundle_id` the http handler and wait for the answer.
///
/// Returns the message to report. A declined or ignored panel is a skip, not a
/// failure: the next run asks again.
pub fn converge(bundle_id: &str, budget: Duration) -> Result<String, ActionError> {
    if is_default(bundle_id) {
        return Ok(format!("{bundle_id} is already the default browser"));
    }
    if headless::is_headless() {
        return Err(ActionError::skipped(
            "default browser skipped (headless; the panel needs a GUI session)",
        ));
    }
    if !is_installed(bundle_id) {
        return Err(ActionError::skipped(format!(
            "{bundle_id} is not installed yet; the cask installs it on this switch and the next run sets it"
        )));
    }
    request(bundle_id)?;
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(2));
        if is_default(bundle_id) {
            return Ok(format!("{bundle_id} is now the default browser"));
        }
    }
    Err(ActionError::skipped(format!(
        "default browser still {}; macOS's confirmation panel was not answered within {}s",
        current_handler().unwrap_or_else(|| "unknown".into()),
        budget.as_secs()
    )))
}

/// Fire the request. macOS raises its own confirmation panel.
pub fn request(bundle_id: &str) -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        macos::set_default_for_scheme(bundle_id, "http")?;
        macos::activate_confirmation_panel();
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle_id;
        Err(ActionError::skipped("default browser is macOS-only"))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::error::{ActionError, ActionResult};
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::string::CFStringRef;
    use std::ffi::c_void;

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;
    }

    // NSWorkspace and NSRunningApplication live in AppKit; the objc runtime
    // resolves the selectors.
    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}
    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    unsafe fn msg_id(receiver: *mut c_void, sel: *mut c_void) -> *mut c_void {
        let msg: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg(receiver, sel)
    }

    unsafe fn msg_id1(
        receiver: *mut c_void,
        sel: *mut c_void,
        arg: *mut c_void,
    ) -> *mut c_void {
        let msg: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg(receiver, sel, arg)
    }

    unsafe fn shared_workspace() -> *mut c_void {
        let cls = objc_getClass(c"NSWorkspace".as_ptr());
        if cls.is_null() {
            return std::ptr::null_mut();
        }
        msg_id(cls, sel_registerName(c"sharedWorkspace".as_ptr()))
    }

    /// CFString is toll-free bridged to NSString, so a CFStringRef is a valid
    /// NSString argument.
    fn ns_string(value: &str) -> CFString {
        CFString::new(value)
    }

    pub fn handler_for_scheme(scheme: &str) -> Option<String> {
        unsafe {
            let cf = ns_string(scheme);
            let handler = LSCopyDefaultHandlerForURLScheme(cf.as_concrete_TypeRef());
            if handler.is_null() {
                return None;
            }
            let id = CFString::wrap_under_create_rule(handler).to_string();
            if id.is_empty() {
                None
            } else {
                Some(id)
            }
        }
    }

    /// Filesystem path of an installed bundle, or None when it is absent.
    pub fn app_path(bundle_id: &str) -> Option<String> {
        unsafe {
            let ws = shared_workspace();
            if ws.is_null() {
                return None;
            }
            let id = ns_string(bundle_id);
            let url = msg_id1(
                ws,
                sel_registerName(c"URLForApplicationWithBundleIdentifier:".as_ptr()),
                id.as_concrete_TypeRef() as *mut c_void,
            );
            if url.is_null() {
                return None;
            }
            let path = msg_id(url, sel_registerName(c"path".as_ptr()));
            if path.is_null() {
                return None;
            }
            let cf: CFStringRef = path as CFStringRef;
            Some(CFString::wrap_under_get_rule(cf).to_string())
        }
    }

    /// `-[NSWorkspace setDefaultApplicationAtURL:toOpenURLsWithScheme:completionHandler:]`
    /// (macOS 12+, the replacement for the deprecated
    /// `LSSetDefaultHandlerForURLScheme`). The completion handler is optional,
    /// so this passes nil and polls the handler instead; the answer only
    /// exists once the user has dealt with the panel anyway. Only `http` is
    /// set: macOS derives `https` and HTML from it and rejects a direct
    /// `https` change.
    pub fn set_default_for_scheme(bundle_id: &str, scheme: &str) -> ActionResult {
        unsafe {
            let ws = shared_workspace();
            if ws.is_null() {
                return Err(ActionError::failed("NSWorkspace unavailable"));
            }
            let id = ns_string(bundle_id);
            let url = msg_id1(
                ws,
                sel_registerName(c"URLForApplicationWithBundleIdentifier:".as_ptr()),
                id.as_concrete_TypeRef() as *mut c_void,
            );
            if url.is_null() {
                return Err(ActionError::failed(format!(
                    "no application bundle registered for {bundle_id}"
                )));
            }
            let cf_scheme = ns_string(scheme);
            let msg: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
                *mut c_void,
                *mut c_void,
            ) = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            msg(
                ws,
                sel_registerName(
                    c"setDefaultApplicationAtURL:toOpenURLsWithScheme:completionHandler:".as_ptr(),
                ),
                url,
                cf_scheme.as_concrete_TypeRef() as *mut c_void,
                std::ptr::null_mut(),
            );
            Ok(())
        }
    }

    /// CoreServicesUIAgent owns the confirmation panel. It normally comes
    /// forward on its own; this makes sure of it so the panel is never left
    /// sitting behind another window.
    pub fn activate_confirmation_panel() {
        unsafe {
            let cls = objc_getClass(c"NSRunningApplication".as_ptr());
            if cls.is_null() {
                return;
            }
            let id = CFString::new("com.apple.coreservices.uiagent");
            let apps = msg_id1(
                cls,
                sel_registerName(c"runningApplicationsWithBundleIdentifier:".as_ptr()),
                id.as_concrete_TypeRef() as *mut c_void,
            );
            if apps.is_null() {
                return;
            }
            let msg_count: unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let count = msg_count(apps, sel_registerName(c"count".as_ptr()));
            let msg_at: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let msg_activate: unsafe extern "C" fn(*mut c_void, *mut c_void, u64) -> bool =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            for i in 0..count {
                let app = msg_at(apps, sel_registerName(c"objectAtIndex:".as_ptr()), i);
                if app.is_null() {
                    continue;
                }
                // NSApplicationActivateIgnoringOtherApps
                msg_activate(
                    app,
                    sel_registerName(c"activateWithOptions:".as_ptr()),
                    1 << 1,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_converge_skips() {
        headless::force(true);
        // A bundle id that is never the handler, so the headless branch is hit.
        let err = converge("com.example.nonexistent", Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, ActionError::Skipped(_)), "{err}");
        headless::clear_force();
    }

    #[test]
    fn absent_bundle_is_not_installed() {
        assert!(!is_installed("com.example.definitely-not-installed"));
    }
}
