use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionError {
    #[error("skipped: {0}")]
    Skipped(String),
    #[error("{0}")]
    Failed(String),
}

pub type ActionResult = Result<(), ActionError>;

impl ActionError {
    pub fn skipped(msg: impl Into<String>) -> Self {
        Self::Skipped(msg.into())
    }

    pub fn failed(msg: impl Into<String>) -> Self {
        Self::Failed(msg.into())
    }
}
