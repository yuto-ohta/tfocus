use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TfocusError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse terraform file: {0}")]
    ParseError(String),

    #[error("Invalid target number selected")]
    InvalidTargetSelection,

    #[error("Terraform command failed: {0}")]
    TerraformError(String),

    #[error("No terraform files found in directory")]
    NoTerraformFiles,

    #[error("Regular expression error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Failed to execute terraform command: {0}")]
    CommandExecutionError(String),

    #[error("Selected resources span multiple directories: {0:?}")]
    MixedWorkingDirectories(Vec<PathBuf>),
}

pub type Result<T> = std::result::Result<T, TfocusError>;
