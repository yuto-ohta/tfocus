use log::{debug, error};
use std::collections::{BTreeSet, HashSet};
use std::env;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::cli::Operation;
use crate::display::Display;
use crate::error::{Result, TfocusError};
use crate::types::Resource;

/// Stores the child process ID for signal handling
static mut CHILD_PID: Option<u32> = None;

/// Main entry point for executing Terraform commands on selected resources
pub fn execute_with_resources(resources: &[Resource], operation: Operation) -> Result<()> {
    let running = setup_signal_handler()?;
    let target_options = create_target_options(resources)?;
    let working_dir = get_working_directory(resources)?;

    let _ = execute_terraform_command(&operation, &target_options, working_dir, running)?;

    Ok(())
}

/// Sets up the Ctrl+C signal handler
fn setup_signal_handler() -> Result<Arc<AtomicBool>> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        unsafe {
            if let Some(pid) = CHILD_PID {
                Display::print_header("\nReceived Ctrl+C, terminating...");
                #[cfg(unix)]
                {
                    use nix::sys::signal::{self, Signal};
                    use nix::unistd::Pid;
                    let _ = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                }
                #[cfg(windows)]
                {
                    // Additional Windows termination logic here if needed.
                    use windows::Win32::Foundation::HANDLE;
                    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess};
                }
            }
        }
    })
    .map_err(|e| TfocusError::CommandExecutionError(e.to_string()))?;

    Ok(running)
}

/// Creates target options for the Terraform command
fn create_target_options(resources: &[Resource]) -> Result<Vec<String>> {
    let mut target_options = Vec::new();
    let mut seen_targets = HashSet::new();

    for resource in resources {
        let target = format!("-target={}", resource.target_string());
        if seen_targets.insert(target.clone()) {
            target_options.push(target);
        }
    }

    if target_options.is_empty() {
        return Err(TfocusError::ParseError("No targets specified".to_string()));
    }

    Ok(target_options)
}

/// Gets the working directory from the first resource
fn get_working_directory(resources: &[Resource]) -> Result<&Path> {
    if resources.is_empty() {
        return Err(TfocusError::ParseError(
            "No resources specified".to_string(),
        ));
    }

    let unique_dirs: BTreeSet<_> = resources
        .iter()
        .map(|resource| {
            resource
                .file_path
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf()
        })
        .collect();

    if unique_dirs.len() > 1 {
        return Err(TfocusError::MixedWorkingDirectories(
            unique_dirs.into_iter().collect(),
        ));
    }

    resources
        .first()
        .map(|resource| resource.file_path.parent().unwrap_or(Path::new(".")))
        .ok_or_else(|| TfocusError::ParseError("No resources specified".to_string()))
}

fn build_command_display(
    terraform_binary: &str,
    operation: &Operation,
    target_options: &[String],
) -> String {
    if target_options.is_empty() {
        format!("{} {}", terraform_binary, operation)
    } else {
        format!(
            "{} {} {}",
            terraform_binary,
            operation,
            target_options.join(" ")
        )
    }
}

/// Executes the Terraform command with the specified options
fn execute_terraform_command(
    operation: &Operation,
    target_options: &[String],
    working_dir: &Path,
    running: Arc<AtomicBool>,
) -> Result<bool> {
    // read `TERRAFORM_BINARY_NAME` env, fallback to "terraform"
    let terraform_binary =
        env::var("TERRAFORM_BINARY_NAME").unwrap_or_else(|_| "terraform".to_string());
    let mut command = Command::new(&terraform_binary);
    command.arg(operation.to_string()).current_dir(working_dir);

    for target in target_options {
        command.arg(target);
    }

    let command_str = build_command_display(&terraform_binary, operation, target_options);

    Display::print_command(&command_str);
    debug!(
        "Executing terraform command in directory: {:?}",
        working_dir
    );
    debug!("Full command: {:?}", command);

    let mut child = command
        .spawn()
        .map_err(|e| TfocusError::CommandExecutionError(e.to_string()))?;

    unsafe {
        CHILD_PID = Some(child.id());
    }

    match child.wait() {
        Ok(status) if status.success() => {
            if running.load(Ordering::SeqCst) {
                debug!("Terraform command executed successfully");
                Display::print_success("Operation completed successfully");
                Ok(true)
            } else {
                Display::print_header("\nOperation cancelled by user");
                Ok(false)
            }
        }
        Ok(status) => {
            let error_msg = format!("Terraform command failed with status: {}", status);
            error!("{}", error_msg);
            Err(TfocusError::TerraformError(error_msg))
        }
        Err(e) => {
            let error_msg = format!("Failed to execute terraform command: {}", e);
            error!("{}", error_msg);
            Err(TfocusError::CommandExecutionError(error_msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_create_target_options() {
        let resources = vec![
            Resource {
                resource_type: "aws_instance".to_string(),
                name: "web".to_string(),
                is_module: false,
                file_path: PathBuf::from("main.tf"),
                has_count: false,
                has_for_each: false,
                index: None,
            },
            Resource {
                resource_type: "aws_instance".to_string(),
                name: "app".to_string(),
                is_module: false,
                file_path: PathBuf::from("main.tf"),
                has_count: true,
                has_for_each: false,
                index: Some("0".to_string()),
            },
        ];

        let options = create_target_options(&resources).unwrap();
        assert_eq!(options[0], "-target=aws_instance.web");
        assert_eq!(options[1], "-target=aws_instance.app[0]");
    }

    #[test]
    fn test_create_target_options_deduplicates() {
        let resources = vec![
            Resource {
                resource_type: "aws_instance".to_string(),
                name: "web".to_string(),
                is_module: false,
                file_path: PathBuf::from("main.tf"),
                has_count: false,
                has_for_each: false,
                index: None,
            },
            Resource {
                resource_type: "aws_instance".to_string(),
                name: "web".to_string(),
                is_module: false,
                file_path: PathBuf::from("main.tf"),
                has_count: false,
                has_for_each: false,
                index: None,
            },
        ];

        let options = create_target_options(&resources).unwrap();
        assert_eq!(options, vec!["-target=aws_instance.web".to_string()]);
    }

    #[test]
    fn test_get_working_directory_with_multiple_directories() {
        let resources = vec![
            Resource {
                resource_type: "aws_instance".to_string(),
                name: "web".to_string(),
                is_module: false,
                file_path: PathBuf::from("dir1/main.tf"),
                has_count: false,
                has_for_each: false,
                index: None,
            },
            Resource {
                resource_type: "aws_instance".to_string(),
                name: "app".to_string(),
                is_module: false,
                file_path: PathBuf::from("dir2/main.tf"),
                has_count: false,
                has_for_each: false,
                index: None,
            },
        ];

        let result = get_working_directory(&resources);
        assert!(matches!(
            result,
            Err(TfocusError::MixedWorkingDirectories(_))
        ));
    }

    #[test]
    fn test_build_command_display_for_plan() {
        let command = build_command_display(
            "terraform",
            &Operation::Plan,
            &["-target=aws_instance.web".to_string()],
        );

        assert_eq!(command, "terraform plan -target=aws_instance.web");
    }

    #[test]
    fn test_build_command_display_for_apply_does_not_include_auto_approve() {
        let command = build_command_display(
            "terraform",
            &Operation::Apply,
            &["-target=aws_instance.web".to_string()],
        );

        assert_eq!(command, "terraform apply -target=aws_instance.web");
        assert!(!command.contains("-auto-approve"));
    }
}
