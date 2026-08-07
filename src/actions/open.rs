use crate::config::{AppEntry, Config};
use crate::error::{ActionError, ActionResult};
use crate::headless;
use std::process::Command;

pub fn open_target(config: &Config, target: &str) -> ActionResult {
    match target.to_ascii_lowercase().as_str() {
        "browser" | "default-browser" | "default_browser" => {
            return open_default_browser();
        }
        _ => {}
    }
    if let Some(entry) = config.resolve_app(target) {
        return open_entry(entry);
    }
    if target.contains('.') {
        return open_bundle_id(target);
    }
    open_by_name(target)
}

/// Open the OS default HTTP(S) handler (no hardcoded browser bundle id).
pub fn open_default_browser() -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        if headless::is_headless() {
            return Err(ActionError::skipped(
                "default browser skipped in headless (no GUI session)",
            ));
        }
        if let Some(bundle_id) = macos_default_http_handler() {
            return open_bundle_id(&bundle_id);
        }
        // Fallback: LaunchServices URL open uses the default handler.
        run_open(&["https://"])
    }
    #[cfg(target_os = "linux")]
    {
        open_linux_url("https://")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(ActionError::skipped("open-default-browser unsupported on this OS"))
    }
}

fn open_entry(entry: &AppEntry) -> ActionResult {
    if let Some(bundle_id) = entry.bundle_id.as_deref() {
        return open_bundle_id(bundle_id);
    }
    if let Some(name) = entry.name.as_deref() {
        return open_by_name(name);
    }
    Err(ActionError::failed(
        "app entry has neither bundle_id nor name",
    ))
}

pub fn open_bundle_id(bundle_id: &str) -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        run_open(&["-b", bundle_id])
    }
    #[cfg(target_os = "linux")]
    {
        let _ = bundle_id;
        if headless::is_headless() {
            return Err(ActionError::skipped(
                "linux open by bundle id skipped in headless (no desktop session)",
            ));
        }
        Err(ActionError::failed(
            "linux open by macOS bundle id is unsupported; pass an executable name",
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = bundle_id;
        Err(ActionError::skipped("open unsupported on this OS"))
    }
}

pub fn open_by_name(name: &str) -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        run_open(&["-a", name])
    }
    #[cfg(target_os = "linux")]
    {
        if headless::is_headless() {
            return run_linux_open(name);
        }
        run_linux_open(name)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = name;
        Err(ActionError::skipped("open unsupported on this OS"))
    }
}

#[cfg(target_os = "macos")]
fn macos_default_http_handler() -> Option<String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::string::CFStringRef;

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn LSCopyDefaultHandlerForURLScheme(inURLScheme: CFStringRef) -> CFStringRef;
    }

    unsafe {
        let scheme = CFString::new("http");
        let handler_ref = LSCopyDefaultHandlerForURLScheme(scheme.as_concrete_TypeRef());
        if handler_ref.is_null() {
            return None;
        }
        let handler = CFString::wrap_under_create_rule(handler_ref);
        let id = handler.to_string();
        if id.is_empty() {
            None
        } else {
            Some(id)
        }
    }
}

#[cfg(target_os = "macos")]
fn run_open(args: &[&str]) -> ActionResult {
    let output = Command::new("open")
        .args(args)
        .output()
        .map_err(|e| ActionError::failed(format!("failed to spawn open: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if headless::is_headless() {
            return Err(ActionError::skipped(format!(
                "open skipped/failed in headless: {stderr}"
            )));
        }
        Err(ActionError::failed(format!("open failed: {stderr}")))
    }
}

#[cfg(target_os = "linux")]
fn open_linux_url(url: &str) -> ActionResult {
    let output = Command::new("xdg-open")
        .arg(url)
        .output()
        .map_err(|e| ActionError::failed(format!("xdg-open: {e}")))?;
    if output.status.success() {
        Ok(())
    } else if headless::is_headless() {
        Err(ActionError::skipped(format!(
            "xdg-open unavailable/failed in headless for {url}"
        )))
    } else {
        Err(ActionError::failed(format!(
            "xdg-open failed for {url}: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

#[cfg(target_os = "linux")]
fn run_linux_open(name: &str) -> ActionResult {
    if which_exists(name) {
        let status = Command::new(name)
            .status()
            .map_err(|e| ActionError::failed(format!("spawn {name}: {e}")))?;
        return if status.success() {
            Ok(())
        } else {
            Err(ActionError::failed(format!("{name} exited {status}")))
        };
    }
    let output = Command::new("xdg-open")
        .arg(name)
        .output()
        .map_err(|e| ActionError::failed(format!("xdg-open: {e}")))?;
    if output.status.success() {
        Ok(())
    } else if headless::is_headless() {
        Err(ActionError::skipped(format!(
            "xdg-open unavailable/failed in headless for {name}"
        )))
    } else {
        Err(ActionError::failed(format!(
            "xdg-open failed for {name}: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

#[cfg(target_os = "linux")]
fn which_exists(name: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn resolve_then_open_unknown_fails_or_skips() {
        headless::force(true);
        let cfg = Config::default();
        let result = open_target(&cfg, "definitely-not-an-app-zzzx");
        assert!(result.is_err());
        headless::clear_force();
    }

    #[test]
    fn browser_aliases_route_to_default_handler_path() {
        headless::force(true);
        let cfg = Config::default();
        // Headless skips; must not resolve as a missing app key.
        let err = open_target(&cfg, "browser").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("headless") || msg.contains("default browser"),
            "unexpected error: {msg}"
        );
        let err2 = open_target(&cfg, "default-browser").unwrap_err();
        assert!(err2.to_string().contains("headless"));
        headless::clear_force();
    }
}
