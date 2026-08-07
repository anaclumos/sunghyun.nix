use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Ok,
    Skipped,
    Failed,
}

impl fmt::Display for StepStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Skipped => write!(f, "skipped"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepReport {
    pub id: String,
    pub status: StepStatus,
    pub message: String,
}

impl StepReport {
    pub fn ok(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: StepStatus::Ok,
            message: message.into(),
        }
    }

    pub fn skipped(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: StepStatus::Skipped,
            message: message.into(),
        }
    }

    pub fn failed(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: StepStatus::Failed,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Report {
    pub headless: bool,
    pub steps: Vec<StepReport>,
}

impl Report {
    pub fn hard_failures(&self) -> impl Iterator<Item = &StepReport> {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Failed)
    }

    pub fn exit_code(&self) -> i32 {
        if self.hard_failures().next().is_some() {
            1
        } else {
            0
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    pub fn to_plain(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("headless={}", self.headless));
        for s in &self.steps {
            lines.push(format!("[{}] {}: {}", s.status, s.id, s.message));
        }
        let fails = self.hard_failures().count();
        let skips = self
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Skipped)
            .count();
        let oks = self
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Ok)
            .count();
        lines.push(format!("summary ok={oks} skipped={skips} failed={fails}"));
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_skips_are_not_failures() {
        let report = Report {
            headless: true,
            steps: vec![
                StepReport::ok("brew", "already installed"),
                StepReport::skipped("mas", "not signed in / headless"),
                StepReport::skipped("spotlight", "headless"),
            ],
        };
        assert_eq!(report.exit_code(), 0);
    }

    #[test]
    fn hard_fail_nonzero() {
        let report = Report {
            headless: false,
            steps: vec![StepReport::failed("brew", "network")],
        };
        assert_eq!(report.exit_code(), 1);
    }
}
