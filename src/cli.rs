use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// The path to the Terraform directory
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,

    /// The operation to perform
    #[arg(short, long)]
    pub operation: Option<Operation>,

    /// Filter selection by type (file, module)
    #[arg(short = 't', long = "type")]
    pub selection_type: Option<SelectionType>,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Non-interactive mode
    #[arg(short, long)]
    pub non_interactive: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SelectionType {
    File,
    Module,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Operation {
    Plan,
    Apply,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Plan => write!(f, "plan"),
            Operation::Apply => write!(f, "apply"),
        }
    }
}
