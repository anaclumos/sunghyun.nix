//! Reserved chords (OUTCOMES.md q).
//!
//! Some chords belong to an app, not to macOS. ⌘⇧Space is 1Password's Quick
//! Access, and macOS 27 ships symbolic hot key 263 ("Ask Siri about active
//! window") on the same chord, so both fire.
//!
//! Two things have to happen and neither one does the other's job. The
//! preference domain is what a fresh login reads, but `AppleSymbolicHotKeys`
//! holds every other system shortcut, so only the one offending key may be
//! touched. The running window server never re-reads that domain, so
//! `CGSSetSymbolicHotKeyEnabled` is what frees the chord in this session
//! without a logout.
//!
//! Claimants are matched by chord, not by identifier: Apple renumbers these
//! between releases, and 263 did not exist before the Siri screenshot actions.

use crate::error::{ActionError, ActionResult};

const SHIFT: u32 = 0x0002_0000;
const CONTROL: u32 = 0x0004_0000;
const OPTION: u32 = 0x0008_0000;
const COMMAND: u32 = 0x0010_0000;
const MODIFIER_MASK: u32 = SHIFT | CONTROL | OPTION | COMMAND;

/// Chords macOS must leave alone, with the app that owns each one.
const RESERVED: &[(&str, u16, u32)] = &[("1Password Quick Access", 49, COMMAND | SHIFT)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimant {
    pub id: i32,
    pub reserved_for: &'static str,
    pub key_equivalent: u16,
    pub virtual_key: u16,
    pub modifiers: u32,
    pub enabled: bool,
}

impl Claimant {
    pub fn describe(&self) -> String {
        format!(
            "symbolic hot key {} claims {} (key {}, modifiers {})",
            self.id, self.reserved_for, self.virtual_key, self.modifiers
        )
    }
}

/// Every system shortcut currently bound to a reserved chord.
pub fn claimants() -> Result<Vec<Claimant>, ActionError> {
    #[cfg(target_os = "macos")]
    {
        if crate::headless::is_headless() {
            return Err(ActionError::skipped(
                "reserved chords skipped in headless (no window server)",
            ));
        }
        macos::scan()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped("symbolic hot keys are macOS only"))
    }
}

/// Free every reserved chord, in this session and at the next login.
pub fn apply() -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        let found = claimants()?;
        for claimant in found.iter().filter(|c| c.enabled) {
            macos::disable_now(claimant.id)?;
            macos::persist_disabled(claimant)?;
            println!("disabled {}", claimant.describe());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped("symbolic hot keys are macOS only"))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{Claimant, MODIFIER_MASK, RESERVED};
    use crate::error::{ActionError, ActionResult};
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::CFTypeRef;
    use core_foundation_sys::string::CFStringRef;
    use std::ffi::CString;
    use std::os::raw::c_void;

    const SKYLIGHT: &str = "/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";
    const DOMAIN: &str = "com.apple.symbolichotkeys";
    const KEY: &str = "AppleSymbolicHotKeys";
    /// Apple's own entries stop well short of this; 263 is the highest in use.
    const MAX_HOT_KEY: i32 = 512;

    type GetValue = unsafe extern "C" fn(i32, *mut u16, *mut u16, *mut u32) -> i32;
    type IsEnabled = unsafe extern "C" fn(i32) -> bool;
    type SetEnabled = unsafe extern "C" fn(i32, bool) -> i32;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFPreferencesCopyValue(
            key: CFStringRef,
            application_id: CFStringRef,
            user_name: CFStringRef,
            host_name: CFStringRef,
        ) -> CFTypeRef;
        fn CFPreferencesSetValue(
            key: CFStringRef,
            value: CFTypeRef,
            application_id: CFStringRef,
            user_name: CFStringRef,
            host_name: CFStringRef,
        );
        fn CFPreferencesSynchronize(
            application_id: CFStringRef,
            user_name: CFStringRef,
            host_name: CFStringRef,
        ) -> bool;
        static kCFPreferencesCurrentUser: CFStringRef;
        static kCFPreferencesAnyHost: CFStringRef;
    }

    fn open_skylight() -> Result<*mut c_void, ActionError> {
        let path = CString::new(SKYLIGHT).expect("static path");
        let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LAZY) };
        if handle.is_null() {
            return Err(ActionError::skipped("SkyLight is unavailable"));
        }
        Ok(handle)
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

    pub fn scan() -> Result<Vec<Claimant>, ActionError> {
        let handle = open_skylight()?;
        let get: GetValue =
            unsafe { std::mem::transmute(sym(handle, "CGSGetSymbolicHotKeyValue")?) };
        let is_enabled: IsEnabled =
            unsafe { std::mem::transmute(sym(handle, "CGSIsSymbolicHotKeyEnabled")?) };

        let mut found = Vec::new();
        for id in 0..MAX_HOT_KEY {
            let (mut key_equivalent, mut virtual_key, mut modifiers) = (0u16, 0u16, 0u32);
            let status = unsafe { get(id, &mut key_equivalent, &mut virtual_key, &mut modifiers) };
            if status != 0 {
                continue;
            }
            let Some((reserved_for, _, _)) = RESERVED.iter().find(|(_, vk, mods)| {
                *vk == virtual_key && (modifiers & MODIFIER_MASK) == *mods
            }) else {
                continue;
            };
            found.push(Claimant {
                id,
                reserved_for,
                key_equivalent,
                virtual_key,
                modifiers,
                enabled: unsafe { is_enabled(id) },
            });
        }
        Ok(found)
    }

    pub fn disable_now(id: i32) -> ActionResult {
        let handle = open_skylight()?;
        let set: SetEnabled = unsafe { std::mem::transmute(sym(handle, "CGSSetSymbolicHotKeyEnabled")?) };
        let is_enabled: IsEnabled =
            unsafe { std::mem::transmute(sym(handle, "CGSIsSymbolicHotKeyEnabled")?) };
        let status = unsafe { set(id, false) };
        if status != 0 {
            return Err(ActionError::failed(format!(
                "CGSSetSymbolicHotKeyEnabled({id}) failed: {status}"
            )));
        }
        if unsafe { is_enabled(id) } {
            return Err(ActionError::failed(format!(
                "symbolic hot key {id} is still enabled after disabling it"
            )));
        }
        Ok(())
    }

    /// Read-modify-write of the one identifier, so every other system shortcut
    /// in `AppleSymbolicHotKeys` survives untouched.
    pub fn persist_disabled(claimant: &Claimant) -> ActionResult {
        let key = CFString::new(KEY);
        let domain = CFString::new(DOMAIN);
        let existing = unsafe {
            CFPreferencesCopyValue(
                key.as_concrete_TypeRef(),
                domain.as_concrete_TypeRef(),
                kCFPreferencesCurrentUser,
                kCFPreferencesAnyHost,
            )
        };
        let mut pairs: Vec<(CFType, CFType)> = Vec::new();
        let id = claimant.id.to_string();
        if !existing.is_null() {
            let dict = unsafe { CFType::wrap_under_create_rule(existing) }
                .downcast_into::<CFDictionary>()
                .ok_or_else(|| ActionError::failed("AppleSymbolicHotKeys is not a dictionary"))?;
            let (keys, values) = dict.get_keys_and_values();
            for (k, v) in keys.into_iter().zip(values) {
                let k = unsafe { CFType::wrap_under_get_rule(k as CFTypeRef) };
                let v = unsafe { CFType::wrap_under_get_rule(v as CFTypeRef) };
                let same = k
                    .downcast::<CFString>()
                    .is_some_and(|s| s.to_string() == id);
                if !same {
                    pairs.push((k, v));
                }
            }
        }
        pairs.push((
            CFString::new(&id).as_CFType(),
            entry(claimant).as_CFType(),
        ));

        let merged = CFDictionary::from_CFType_pairs(&pairs);
        unsafe {
            CFPreferencesSetValue(
                key.as_concrete_TypeRef(),
                merged.as_CFTypeRef(),
                domain.as_concrete_TypeRef(),
                kCFPreferencesCurrentUser,
                kCFPreferencesAnyHost,
            );
            if !CFPreferencesSynchronize(
                domain.as_concrete_TypeRef(),
                kCFPreferencesCurrentUser,
                kCFPreferencesAnyHost,
            ) {
                return Err(ActionError::failed(
                    "cfprefsd refused to synchronize com.apple.symbolichotkeys",
                ));
            }
        }
        Ok(())
    }

    /// Same shape System Settings writes when the owner unticks a shortcut.
    fn entry(claimant: &Claimant) -> CFDictionary<CFType, CFType> {
        let parameters = CFArray::from_CFTypes(&[
            CFNumber::from(i64::from(claimant.key_equivalent)).as_CFType(),
            CFNumber::from(i64::from(claimant.virtual_key)).as_CFType(),
            CFNumber::from(i64::from(claimant.modifiers)).as_CFType(),
        ]);
        let value: CFDictionary<CFType, CFType> = CFDictionary::from_CFType_pairs(&[
            (
                CFString::new("parameters").as_CFType(),
                parameters.as_CFType(),
            ),
            (
                CFString::new("type").as_CFType(),
                CFString::new("standard").as_CFType(),
            ),
        ]);
        CFDictionary::from_CFType_pairs(&[
            (
                CFString::new("enabled").as_CFType(),
                CFBoolean::false_value().as_CFType(),
            ),
            (CFString::new("value").as_CFType(), value.as_CFType()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_shift_space_is_the_reserved_chord() {
        let (name, key, modifiers) = RESERVED[0];
        assert_eq!(name, "1Password Quick Access");
        assert_eq!(key, 49);
        assert_eq!(modifiers, COMMAND | SHIFT);
        // Control adds a bit, so ⌃⌘⇧Space must not look like the same chord.
        assert_ne!(modifiers, COMMAND | SHIFT | CONTROL);
    }

    #[test]
    fn claimants_are_command_shift_space_only() {
        match claimants() {
            Ok(found) => {
                for c in found {
                    assert_eq!(c.virtual_key, 49);
                    assert_eq!(c.modifiers & MODIFIER_MASK, COMMAND | SHIFT);
                }
            }
            Err(ActionError::Skipped(_)) => {}
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
