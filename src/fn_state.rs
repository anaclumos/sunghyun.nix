//! Top-row fn behaviour (OUTCOMES.md o).
//!
//! `system.defaults` writes `com.apple.keyboard.fnState` into
//! `.GlobalPreferences`, but IOHIDSystem only reads that preference into its
//! `HIDParameters` at login. On a session that is already running (every
//! `darwin-rebuild switch`, including the first one `install.sh` performs) the
//! driver keeps the old `HIDFKeyMode`, so the top row and Karabiner's
//! `system.use_fkeys_as_standard_function_keys` both stay on the old
//! behaviour until the next logout. Pushing the parameter back into
//! IOHIDSystem is what makes the declared state converge immediately.
//!
//! `hidutil` cannot do this: it addresses HID devices, and IOHIDSystem is not
//! one, so `hidutil property --set '{"HIDFKeyMode":N}'` reports success and
//! changes nothing.

use crate::error::{ActionError, ActionResult};

#[cfg(target_os = "macos")]
const HID_F_KEY_MODE: &str = "HIDFKeyMode";
#[cfg(target_os = "macos")]
const HID_PARAMETERS: &str = "HIDParameters";

/// The `HIDFKeyMode` IOHIDSystem is currently enforcing, if it can be read.
pub fn current_mode() -> Result<i64, ActionError> {
    #[cfg(target_os = "macos")]
    {
        macos::read_mode()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped("fn key mode is macOS only"))
    }
}

/// Make IOHIDSystem enforce `standard_function_keys` now.
///
/// `true` means "Use F1, F2, etc. keys as standard function keys", matching
/// the sense of `com.apple.keyboard.fnState`.
pub fn apply(standard_function_keys: bool) -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        let wanted = i64::from(standard_function_keys);
        if macos::read_mode() == Ok(wanted) {
            return Ok(());
        }
        macos::write_mode(wanted)?;
        match macos::read_mode() {
            Ok(actual) if actual == wanted => Ok(()),
            Ok(actual) => Err(ActionError::failed(format!(
                "IOHIDSystem accepted HIDFKeyMode={wanted} but still reports {actual}"
            ))),
            Err(e) => Err(e),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = standard_function_keys;
        Err(ActionError::skipped("fn key mode is macOS only"))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{HID_F_KEY_MODE, HID_PARAMETERS};
    use crate::error::{ActionError, ActionResult};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::{kCFAllocatorDefault, CFTypeRef};
    use core_foundation_sys::dictionary::CFDictionaryRef;
    use core_foundation_sys::string::CFStringRef;
    use std::ffi::CString;
    use std::os::raw::{c_char, c_void};

    type IoObject = u32;
    type KernReturn = i32;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOServiceMatching(name: *const c_char) -> CFDictionaryRef;
        fn IOServiceGetMatchingService(main_port: u32, matching: CFDictionaryRef) -> IoObject;
        fn IORegistryEntrySetCFProperty(
            entry: IoObject,
            property_name: CFStringRef,
            property: CFTypeRef,
        ) -> KernReturn;
        fn IORegistryEntryCreateCFProperty(
            entry: IoObject,
            key: CFStringRef,
            allocator: CFTypeRef,
            options: u32,
        ) -> CFTypeRef;
        fn IOObjectRelease(object: IoObject) -> KernReturn;
    }

    struct Service(IoObject);

    impl Drop for Service {
        fn drop(&mut self) {
            unsafe { IOObjectRelease(self.0) };
        }
    }

    fn open_service() -> Result<Service, ActionError> {
        let name = CString::new("IOHIDSystem").expect("static name");
        // IOServiceGetMatchingService consumes the matching dictionary, and a
        // null main port means "the default port".
        let service = unsafe {
            let matching = IOServiceMatching(name.as_ptr());
            if matching.is_null() {
                return Err(ActionError::failed("IOServiceMatching(IOHIDSystem) failed"));
            }
            IOServiceGetMatchingService(0, matching)
        };
        if service == 0 {
            return Err(ActionError::skipped("IOHIDSystem is not present"));
        }
        Ok(Service(service))
    }

    pub fn read_mode() -> Result<i64, ActionError> {
        let service = open_service()?;
        let key = CFString::new(HID_PARAMETERS);
        let raw = unsafe {
            IORegistryEntryCreateCFProperty(
                service.0,
                key.as_concrete_TypeRef(),
                kCFAllocatorDefault as CFTypeRef,
                0,
            )
        };
        if raw.is_null() {
            return Err(ActionError::failed("IOHIDSystem has no HIDParameters"));
        }
        // Only the untyped CFDictionary is a ConcreteCFType, so look the key up
        // by raw pointer and re-wrap the value.
        let params = unsafe { CFType::wrap_under_create_rule(raw) }
            .downcast_into::<CFDictionary>()
            .ok_or_else(|| ActionError::failed("HIDParameters is not a dictionary"))?;
        let mode_key = CFString::new(HID_F_KEY_MODE);
        params
            .find(mode_key.as_CFTypeRef() as *const c_void)
            .map(|v| unsafe { CFType::wrap_under_get_rule(*v as CFTypeRef) })
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
            .ok_or_else(|| ActionError::failed("HIDParameters has no numeric HIDFKeyMode"))
    }

    pub fn write_mode(mode: i64) -> ActionResult {
        let service = open_service()?;
        let value = CFNumber::from(mode as i32);
        let status = unsafe {
            IORegistryEntrySetCFProperty(
                service.0,
                CFString::new(HID_F_KEY_MODE).as_concrete_TypeRef(),
                value.as_CFTypeRef(),
            )
        };
        if status != 0 {
            return Err(ActionError::failed(format!(
                "IORegistryEntrySetCFProperty(HIDFKeyMode) failed: {status}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_mode_is_zero_or_one_when_readable() {
        match current_mode() {
            Ok(mode) => assert!(mode == 0 || mode == 1, "unexpected HIDFKeyMode {mode}"),
            Err(ActionError::Skipped(_)) => {}
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
