use crate::config::Config;
use crate::error::{ActionError, ActionResult};
use crate::headless;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TileAction {
    Left,
    Right,
    Top,
    Bottom,
    Center,
    TopLeft,
    FirstFourth,
    SecondFourth,
    ThirdFourth,
    LastFourth,
    LastThreeFourths,
    /// Fill the visible desktop (not macOS native fullscreen).
    Maximize,
    /// Rightmost third of the screen (alternate to last-three-fourths).
    RightThird,
    Fullscreen,
}

impl TileAction {
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "left" | "left-half" => Some(Self::Left),
            "right" | "right-half" => Some(Self::Right),
            "top" | "top-half" => Some(Self::Top),
            "bottom" | "bottom-half" => Some(Self::Bottom),
            "center" => Some(Self::Center),
            "top-left" | "top-left-quarter" => Some(Self::TopLeft),
            "first-fourth" | "1" => Some(Self::FirstFourth),
            "second-fourth" | "2" => Some(Self::SecondFourth),
            "third-fourth" | "3" => Some(Self::ThirdFourth),
            "last-fourth" | "4" => Some(Self::LastFourth),
            "last-three-fourths" => Some(Self::LastThreeFourths),
            "maximize" | "max" => Some(Self::Maximize),
            "right-third" | "last-third" => Some(Self::RightThird),
            "fullscreen" | "toggle-fullscreen" => Some(Self::Fullscreen),
            _ => None,
        }
    }

    pub fn inventory_name(self) -> &'static str {
        match self {
            Self::Left => "Left Half",
            Self::Right => "Right Half",
            Self::Top => "Top Half",
            Self::Bottom => "Bottom Half",
            Self::Center => "Center",
            Self::TopLeft => "Top Left Quarter",
            Self::FirstFourth => "First Fourth",
            Self::SecondFourth => "Second Fourth",
            Self::ThirdFourth => "Third Fourth",
            Self::LastFourth => "Last Fourth",
            Self::LastThreeFourths => "Last Three Fourths",
            Self::Maximize => "Maximize",
            Self::RightThird => "Right Third",
            Self::Fullscreen => "Toggle Fullscreen",
        }
    }
}

pub fn tile(config: &Config, action_name: &str) -> ActionResult {
    let Some(action) = TileAction::parse(action_name) else {
        return Err(ActionError::failed(format!("unknown tile action: {action_name}")));
    };
    tile_action(config, action)
}

pub fn tile_action(config: &Config, action: TileAction) -> ActionResult {
    if headless::is_headless() {
        return Err(ActionError::skipped(format!(
            "tile {} skipped (headless)",
            action.inventory_name()
        )));
    }

    #[cfg(target_os = "macos")]
    {
        macos_tile(config.tiles.gap, action)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = config;
        let _ = action;
        Err(ActionError::skipped(
            "window tiling is macOS-only in v1; compositor backends later",
        ))
    }
}

#[cfg(target_os = "macos")]
fn macos_tile(gap: i64, action: TileAction) -> ActionResult {
    // Native AX API, not osascript: the TCC decision then keys off this
    // binary's own Accessibility grant, independent of the spawning process
    // (Karabiner's console_user_server, a terminal, kanata, ...). An
    // osascript child would be attributed to the responsible process of the
    // whole chain and fail under Karabiner even with the binary granted.
    if !crate::ax::is_process_trusted() {
        return Err(ActionError::failed(
            "Accessibility not granted for sunghyun (required for tile); run `sunghyun post-switch` to open the Settings pane",
        ));
    }

    let (fx, fy, fw, fh) = match action {
        TileAction::Left => (0.0, 0.0, 0.5, 1.0),
        TileAction::Right => (0.5, 0.0, 0.5, 1.0),
        TileAction::Top => (0.0, 0.0, 1.0, 0.5),
        TileAction::Bottom => (0.0, 0.5, 1.0, 0.5),
        TileAction::Center => (0.125, 0.125, 0.75, 0.75),
        TileAction::TopLeft => (0.0, 0.0, 0.5, 0.5),
        TileAction::FirstFourth => (0.0, 0.0, 0.25, 1.0),
        TileAction::SecondFourth => (0.25, 0.0, 0.25, 1.0),
        TileAction::ThirdFourth => (0.5, 0.0, 0.25, 1.0),
        TileAction::LastFourth => (0.75, 0.0, 0.25, 1.0),
        TileAction::LastThreeFourths => (0.25, 0.0, 0.75, 1.0),
        TileAction::Maximize => (0.0, 0.0, 1.0, 1.0),
        TileAction::RightThird => (2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
        TileAction::Fullscreen => {
            return ax_api::toggle_fullscreen();
        }
    };

    let screen = ax_api::main_display_bounds();
    let g = gap as f64;
    let x = screen.origin.x + screen.size.width * fx + g;
    let y = screen.origin.y + screen.size.height * fy + g;
    let w = screen.size.width * fw - g * 2.0;
    let h = screen.size.height * fh - g * 2.0;
    ax_api::set_focused_window_frame(x, y, w, h)
}

/// Thin FFI over ApplicationServices AXUIElement + CoreGraphics display
/// bounds. Same style as ax.rs (hand-declared externs on the stable C API).
#[cfg(target_os = "macos")]
mod ax_api {
    use crate::error::{ActionError, ActionResult};
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::string::CFString;
    use core_foundation_sys::string::CFStringRef;
    use std::ptr;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct CGPoint {
        pub x: f64,
        pub y: f64,
    }
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct CGSize {
        pub width: f64,
        pub height: f64,
    }
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct CGRect {
        pub origin: CGPoint,
        pub size: CGSize,
    }

    type AXUIElementRef = CFTypeRef;
    type AXValueRef = CFTypeRef;
    type AXError = i32;
    const K_AX_VALUE_CGPOINT: u32 = 1;
    const K_AX_VALUE_CGSIZE: u32 = 2;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXUIElementSetAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: CFTypeRef,
        ) -> AXError;
        fn AXValueCreate(the_type: u32, value_ptr: *const std::ffi::c_void) -> AXValueRef;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayBounds(display: u32) -> CGRect;
    }

    // NSWorkspace lives in AppKit; the objc runtime resolves the rest.
    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}
    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn objc_msgSend();
    }

    pub fn main_display_bounds() -> CGRect {
        unsafe { CGDisplayBounds(CGMainDisplayID()) }
    }

    fn ax_err(context: &str, code: AXError) -> ActionError {
        let hint = match code {
            -25200 => " (kAXErrorFailure: window rejected the change; native-fullscreen windows cannot be moved/resized)",
            -25204 => " (kAXErrorAPIDisabled: Accessibility not granted for this binary)",
            -25205 => " (kAXErrorNoValue)",
            -25211 => " (kAXErrorNotImplemented by the target app)",
            -25202 => " (kAXErrorInvalidUIElement: no focused window?)",
            _ => "",
        };
        ActionError::failed(format!("{context}: AXError {code}{hint}"))
    }

    fn is_native_fullscreen(window: AXUIElementRef) -> bool {
        match copy_attr(window, "AXFullScreen") {
            Ok(v) => {
                let b: bool =
                    unsafe { CFBoolean::wrap_under_create_rule(v as *const _) }.into();
                b
            }
            Err(_) => false,
        }
    }

    fn copy_attr(element: AXUIElementRef, name: &str) -> Result<CFTypeRef, ActionError> {
        let attr = CFString::new(name);
        let mut out: CFTypeRef = ptr::null();
        let err = unsafe {
            AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut out)
        };
        if err != 0 || out.is_null() {
            return Err(ax_err(&format!("read {name}"), err));
        }
        Ok(out)
    }

    /// Frontmost (keyboard-focused) app pid via NSWorkspace. A window-list
    /// z-order heuristic is wrong here: overlay windows (cua-driver's agent
    /// cursor, HUDs) sit at layer 0 above the active app and are AX-dead
    /// (kAXErrorCannotComplete, seen live 2026-08-08).
    fn frontmost_app_pid() -> Result<i32, ActionError> {
        use std::ffi::c_void;
        unsafe {
            let cls = objc_getClass(c"NSWorkspace".as_ptr());
            if cls.is_null() {
                return Err(ActionError::failed("NSWorkspace class unavailable"));
            }
            let msg_id: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let ws = msg_id(cls, sel_registerName(c"sharedWorkspace".as_ptr()));
            if ws.is_null() {
                return Err(ActionError::failed("NSWorkspace.sharedWorkspace is nil"));
            }
            let app = msg_id(ws, sel_registerName(c"frontmostApplication".as_ptr()));
            if app.is_null() {
                return Err(ActionError::failed("no frontmost application"));
            }
            let msg_pid: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32 =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let pid = msg_pid(app, sel_registerName(c"processIdentifier".as_ptr()));
            if pid <= 0 {
                return Err(ActionError::failed("frontmost application has no pid"));
            }
            Ok(pid)
        }
    }

    /// Frontmost application (via window list) → focused window. Caller
    /// releases both. The system-wide element's AXFocusedApplication is NOT
    /// used: it returns kAXErrorAPIDisabled on macOS 26/27 even for a fully
    /// trusted process (verified live 2026-08-08 with tccd approving the
    /// request), while per-app AXUIElementCreateApplication works.
    fn focused_window() -> Result<(AXUIElementRef, AXUIElementRef), ActionError> {
        let pid = frontmost_app_pid()?;
        let app = unsafe { AXUIElementCreateApplication(pid) };
        if app.is_null() {
            return Err(ActionError::failed("AXUIElementCreateApplication failed"));
        }
        let window = match copy_attr(app, "AXFocusedWindow") {
            Ok(v) => v,
            Err(e) => {
                unsafe { CFRelease(app) };
                return Err(e);
            }
        };
        Ok((app, window))
    }

    fn set_ax_value(
        window: AXUIElementRef,
        name: &str,
        value_type: u32,
        value_ptr: *const std::ffi::c_void,
    ) -> ActionResult {
        let ax_value = unsafe { AXValueCreate(value_type, value_ptr) };
        if ax_value.is_null() {
            return Err(ActionError::failed(format!("AXValueCreate failed for {name}")));
        }
        let attr = CFString::new(name);
        let err = unsafe {
            AXUIElementSetAttributeValue(window, attr.as_concrete_TypeRef(), ax_value)
        };
        unsafe { CFRelease(ax_value) };
        if err != 0 {
            return Err(ax_err(&format!("set {name}"), err));
        }
        Ok(())
    }

    pub fn set_focused_window_frame(x: f64, y: f64, w: f64, h: f64) -> ActionResult {
        let (app, window) = focused_window()?;
        if is_native_fullscreen(window) {
            unsafe {
                CFRelease(window);
                CFRelease(app);
            }
            return Err(ActionError::skipped(
                "focused window is native fullscreen; exit fullscreen (or use tile fullscreen) to tile",
            ));
        }
        let point = CGPoint { x, y };
        let size = CGSize { width: w, height: h };
        // Position before and after size: apps clamp position to the current
        // size, so a grow-then-move (or move-then-grow) can land off-target.
        let result = set_ax_value(
            window,
            "AXPosition",
            K_AX_VALUE_CGPOINT,
            &point as *const _ as *const _,
        )
        .and_then(|_| {
            set_ax_value(window, "AXSize", K_AX_VALUE_CGSIZE, &size as *const _ as *const _)
        })
        .and_then(|_| {
            set_ax_value(
                window,
                "AXPosition",
                K_AX_VALUE_CGPOINT,
                &point as *const _ as *const _,
            )
        });
        unsafe {
            CFRelease(window);
            CFRelease(app);
        }
        result
    }

    pub fn toggle_fullscreen() -> ActionResult {
        let (app, window) = focused_window()?;
        let result = (|| {
            let current = copy_attr(window, "AXFullScreen")?;
            // Copy rule: the wrapper takes ownership and releases on drop.
            let is_fullscreen: bool =
                unsafe { CFBoolean::wrap_under_create_rule(current as *const _) }.into();
            let target = if is_fullscreen {
                CFBoolean::false_value()
            } else {
                CFBoolean::true_value()
            };
            let attr = CFString::new("AXFullScreen");
            let err = unsafe {
                AXUIElementSetAttributeValue(
                    window,
                    attr.as_concrete_TypeRef(),
                    target.as_CFTypeRef(),
                )
            };
            if err != 0 {
                return Err(ax_err("set AXFullScreen", err));
            }
            Ok(())
        })();
        unsafe {
            CFRelease(window);
            CFRelease(app);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn parse_inventory_names() {
        assert_eq!(TileAction::parse("left-half"), Some(TileAction::Left));
        assert_eq!(TileAction::parse("right"), Some(TileAction::Right));
        assert_eq!(
            TileAction::parse("last-three-fourths"),
            Some(TileAction::LastThreeFourths)
        );
        assert_eq!(TileAction::parse("maximize"), Some(TileAction::Maximize));
        assert_eq!(
            TileAction::parse("right-third"),
            Some(TileAction::RightThird)
        );
        assert_eq!(TileAction::parse("nope"), None);
    }

    #[test]
    fn headless_tile_skips() {
        headless::force(true);
        let cfg = Config::default();
        let err = tile(&cfg, "left").unwrap_err();
        assert!(matches!(err, ActionError::Skipped(_)));
    }

    #[test]
    fn no_osascript_in_tile_path() {
        // Regression contract (2026-08-08): tiling must stay on the native AX
        // API. An osascript child re-enters TCC attribution through the
        // spawning chain and breaks under Karabiner shell_command.
        let source = include_str!("tile.rs");
        assert!(!source.contains("osascript\""));
    }
}
