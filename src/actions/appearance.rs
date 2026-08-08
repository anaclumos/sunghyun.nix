//! System appearance (OUTCOMES.md p).
//!
//! Toggles via SkyLight's `SLSSetAppearanceThemeNotifying` rather than an
//! Apple Event to System Events. The AppleScript route is the commonly cited
//! one, but sending it needs `kTCCServiceAppleEvents` for whatever process
//! sends it: a second consent prompt and a second privacy row, separate from
//! the Accessibility grant the binary already holds. SkyLight talks to the
//! window server directly, so a keystroke never has to clear a TCC gate.

use crate::error::{ActionError, ActionResult};
use crate::headless;

pub fn toggle() -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        if headless::is_headless() {
            return Err(ActionError::skipped(
                "appearance skipped in headless (no window server)",
            ));
        }
        let sky = macos::SkyLight::open()?;
        let wanted = !sky.is_dark();
        sky.set_dark(wanted);
        if sky.is_dark() == wanted {
            Ok(())
        } else {
            Err(ActionError::failed(format!(
                "window server kept appearance dark={} after asking for {wanted}",
                sky.is_dark()
            )))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped(
            "appearance toggle is macOS only (SkyLight)",
        ))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::error::ActionError;
    use std::ffi::CString;
    use std::os::raw::c_void;

    const SKYLIGHT: &str = "/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";

    pub struct SkyLight {
        get: unsafe extern "C" fn() -> bool,
        set: unsafe extern "C" fn(bool, bool),
    }

    impl SkyLight {
        pub fn open() -> Result<Self, ActionError> {
            let path = CString::new(SKYLIGHT).expect("static path");
            let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LAZY) };
            if handle.is_null() {
                return Err(ActionError::skipped("SkyLight is unavailable"));
            }
            let get = sym(handle, "SLSGetAppearanceThemeLegacy")?;
            let set = sym(handle, "SLSSetAppearanceThemeNotifying")?;
            Ok(Self {
                get: unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> bool>(get) },
                set: unsafe {
                    std::mem::transmute::<*mut c_void, unsafe extern "C" fn(bool, bool)>(set)
                },
            })
        }

        pub fn is_dark(&self) -> bool {
            unsafe { (self.get)() }
        }

        /// The second argument makes the window server post the appearance
        /// change, which is what repaints running apps and the menu bar.
        pub fn set_dark(&self, dark: bool) {
            unsafe { (self.set)(dark, true) }
        }
    }

    fn sym(handle: *mut c_void, name: &str) -> Result<*mut c_void, ActionError> {
        let symbol = CString::new(name).expect("static symbol");
        let ptr = unsafe { libc::dlsym(handle, symbol.as_ptr()) };
        if ptr.is_null() {
            return Err(ActionError::failed(format!(
                "SkyLight has no {name} on this macOS"
            )));
        }
        Ok(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_skips_instead_of_failing() {
        headless::force(true);
        let err = toggle().unwrap_err();
        assert!(matches!(err, ActionError::Skipped(_)));
        headless::clear_force();
    }
}
