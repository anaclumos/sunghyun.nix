use crate::config::Config;
use crate::error::{ActionError, ActionResult};
use crate::headless;

pub fn switch(config: &Config, name: &str) -> ActionResult {
    let Some(id) = config.resolve_ime_id(name) else {
        return Err(ActionError::failed(format!("unknown input source: {name}")));
    };
    select_by_id(&id)
}

pub fn select_by_id(source_id: &str) -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        macos_select(source_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = source_id;
        Err(ActionError::skipped(
            "input-source is macOS TIS only; skipped on this OS",
        ))
    }
}

#[cfg(target_os = "macos")]
fn macos_select(source_id: &str) -> ActionResult {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::{CFRelease, CFTypeRef};
    use core_foundation_sys::dictionary::CFDictionaryRef;
    use core_foundation_sys::string::CFStringRef;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn TISCreateInputSourceList(
            properties: CFDictionaryRef,
            include_all_installed: u8,
        ) -> CFTypeRef;
        fn TISSelectInputSource(source: CFTypeRef) -> i32;
        static kTISPropertyInputSourceID: CFStringRef;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, idx: isize) -> CFTypeRef;
    }

    let id_string = CFString::new(source_id);
    let key = unsafe { CFString::wrap_under_get_rule(kTISPropertyInputSourceID) };
    let filter: CFDictionary<CFString, CFType> =
        CFDictionary::from_CFType_pairs(&[(key, id_string.as_CFType())]);

    unsafe {
        let list_ref = TISCreateInputSourceList(filter.as_concrete_TypeRef(), 1);
        if list_ref.is_null() {
            if headless::is_headless() {
                return Err(ActionError::skipped(format!(
                    "TIS list empty for {source_id} (headless)"
                )));
            }
            return Err(ActionError::failed(format!(
                "no input source found for id {source_id}"
            )));
        }

        let count = CFArrayGetCount(list_ref);
        if count < 1 {
            CFRelease(list_ref);
            if headless::is_headless() {
                return Err(ActionError::skipped(format!(
                    "TIS source {source_id} not installed (headless)"
                )));
            }
            return Err(ActionError::failed(format!(
                "input source not installed: {source_id}"
            )));
        }

        let source = CFArrayGetValueAtIndex(list_ref, 0);
        if source.is_null() {
            CFRelease(list_ref);
            return Err(ActionError::failed("null TIS source ref"));
        }

        let status = TISSelectInputSource(source);
        CFRelease(list_ref);

        if status == 0 {
            Ok(())
        } else if headless::is_headless() {
            Err(ActionError::skipped(format!(
                "TISSelectInputSource status {status} for {source_id} (headless)"
            )))
        } else {
            Err(ActionError::failed(format!(
                "TISSelectInputSource failed with status {status} for {source_id}"
            )))
        }
    }
}

pub fn map_name_to_id(config: &Config, name: &str) -> Option<String> {
    config.resolve_ime_id(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn ime_id_mapping() {
        let cfg = Config::default();
        assert_eq!(
            map_name_to_id(&cfg, "ABC").as_deref(),
            Some("com.apple.keylayout.ABC")
        );
        assert_eq!(
            map_name_to_id(&cfg, "2SetKorean").as_deref(),
            Some("com.apple.inputmethod.Korean.2SetKorean")
        );
        assert_eq!(map_name_to_id(&cfg, "nope"), None);
    }

    #[test]
    fn headless_switch_does_not_panic() {
        headless::force(true);
        let cfg = Config::default();
        let _ = switch(&cfg, "ABC");
    }
}
