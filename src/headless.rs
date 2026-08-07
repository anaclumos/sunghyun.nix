use std::cell::Cell;
use std::env;

thread_local! {
    static FORCED: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Force headless mode for the current thread (tests / CLI `--headless`).
pub fn force(value: bool) {
    FORCED.with(|c| c.set(Some(value)));
}

pub fn clear_force() {
    FORCED.with(|c| c.set(None));
}

pub fn is_headless() -> bool {
    if let Some(v) = FORCED.with(|c| c.get()) {
        return v;
    }
    if env_flag("SUNGHYUN_HEADLESS") {
        return true;
    }
    detect_auto()
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(v) => {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

fn detect_auto() -> bool {
    #[cfg(target_os = "linux")]
    {
        return env::var_os("DISPLAY").is_none() && env::var_os("WAYLAND_DISPLAY").is_none();
    }

    #[cfg(target_os = "macos")]
    {
        return !has_window_server_session();
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        true
    }
}

#[cfg(target_os = "macos")]
fn has_window_server_session() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation_sys::dictionary::CFDictionaryRef;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
    }

    unsafe {
        let dict_ref = CGSessionCopyCurrentDictionary();
        if dict_ref.is_null() {
            return false;
        }
        let _dict = CFDictionary::<
            core_foundation::string::CFString,
            core_foundation::base::CFType,
        >::wrap_under_create_rule(dict_ref);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_toggles() {
        force(true);
        assert!(is_headless());
        force(false);
        assert!(!is_headless());
        clear_force();
    }
}
