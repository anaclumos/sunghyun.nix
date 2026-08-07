//! Virtualization detection: the single source of truth for "this machine is
//! a VM" across the whole system (CLI reports and the mas convergence
//! LaunchDaemon, which shells out to `sunghyun virt`).
//!
//! Owner policy (2026-08-08): a virtualization end-to-end demo skips the Mac
//! App Store surface *by design*, not by timeout. Signing a throwaway guest
//! into a real Apple Account is not something an e2e run may ever do, so mas
//! must report a skip that names virtualization, never launch App Store, and
//! never schedule background convergence.
//!
//! Signals (both read-only sysctls, no privileges, no GUI):
//!   * `kern.hv_vmm_present` — 1 when a hypervisor is present. Set by the
//!     kernel under Apple's Virtualization.framework (tart/UTM/Parallels use
//!     it) and under other hypervisors.
//!   * `hw.model` — Apple's Virtualization.framework reports `VirtualMac2,1`;
//!     real hardware reports e.g. `Mac17,6`.
//! `hv_vmm_present` is primary; the model string is a second, independent
//! witness so a single sysctl going away cannot silently turn the gate off.

use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Virtualization {
    /// Bare metal (or nothing detectable).
    None,
    /// Running inside a VM; the string names the signal that fired.
    Guest(String),
}

impl Virtualization {
    pub fn is_guest(&self) -> bool {
        matches!(self, Self::Guest(_))
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Guest(r) => Some(r),
            Self::None => None,
        }
    }
}

fn sysctl(key: &str) -> Option<String> {
    let out = Command::new("/usr/sbin/sysctl").args(["-n", key]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

pub fn detect() -> Virtualization {
    if cfg!(not(target_os = "macos")) {
        return Virtualization::None;
    }
    let hv = sysctl("kern.hv_vmm_present");
    let model = sysctl("hw.model");
    let hv_set = hv.as_deref() == Some("1");
    let model_virtual = model
        .as_deref()
        .map(|m| m.starts_with("VirtualMac"))
        .unwrap_or(false);
    if hv_set || model_virtual {
        let mut parts = Vec::new();
        if hv_set {
            parts.push("kern.hv_vmm_present=1".to_string());
        }
        if let Some(m) = model.as_deref() {
            if model_virtual {
                parts.push(format!("hw.model={m}"));
            }
        }
        return Virtualization::Guest(parts.join(", "));
    }
    Virtualization::None
}

/// One-line human summary used by `sunghyun virt` and the verify report.
pub fn describe() -> String {
    match detect() {
        Virtualization::Guest(reason) => {
            format!("virtual machine ({reason}); App Store / mas surfaces skip by design")
        }
        Virtualization::None => {
            let model = sysctl("hw.model").unwrap_or_else(|| "unknown".into());
            format!("physical machine (hw.model={model})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_is_total_and_consistent() {
        let v = detect();
        assert_eq!(v.is_guest(), v.reason().is_some());
        // Never panics, always yields a non-empty description.
        assert!(!describe().is_empty());
    }

    #[test]
    fn guest_reason_names_the_signal() {
        let g = Virtualization::Guest("kern.hv_vmm_present=1".into());
        assert!(g.is_guest());
        assert!(g.reason().unwrap().contains("hv_vmm_present"));
    }
}
