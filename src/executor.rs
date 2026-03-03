use log::{debug, error};
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
pub fn execute_with_resources(resources: &[Resource]) -> Result<()> {
    let running = setup_signal_handler()?;
    let target_options = create_target_options(resources)?;
    let working_dir = get_working_directory(resources)?;

    // Always run plan first
    let plan_succeeded =
        execute_terraform_command(&Operation::Plan, &target_options, working_dir, running.clone())?;

    // Only show apply confirmation if plan succeeded
    if plan_succeeded {
        confirm_and_apply(&target_options, working_dir, running)?;
    }

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
    let target_options: Vec<String> = resources
        .iter()
        .map(|r| format!("-target={}", r.target_string()))
        .collect();

    if target_options.is_empty() {
        return Err(TfocusError::ParseError("No targets specified".to_string()));
    }

    Ok(target_options)
}

/// Gets the working directory from the first resource
fn get_working_directory(resources: &[Resource]) -> Result<&Path> {
    resources
        .first()
        .map(|r| r.file_path.parent().unwrap_or(Path::new(".")))
        .ok_or_else(|| TfocusError::ParseError("No resources specified".to_string()))
}

/// Prompts the user to confirm and apply the planned changes
fn confirm_and_apply(
    target_options: &[String],
    working_dir: &Path,
    running: Arc<AtomicBool>,
) -> Result<()> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    use crossterm::terminal;
    use std::io::Write;

    print!("\nApply these changes? [y/N]: ");
    std::io::stdout().flush().ok();

    terminal::enable_raw_mode()?;
    let apply = loop {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match (key.code, key.modifiers) {
                (KeyCode::Char('y'), KeyModifiers::NONE)
                | (KeyCode::Char('Y'), KeyModifiers::NONE) => break true,
                (KeyCode::Char('c'), KeyModifiers::CONTROL)
                | (KeyCode::Esc, _)
                | (KeyCode::Enter, _)
                | (KeyCode::Char('n'), _)
                | (KeyCode::Char('N'), _) => break false,
                _ => {}
            }
        }
    };
    terminal::disable_raw_mode()?;
    println!();

    if apply {
        execute_terraform_command(&Operation::Apply, target_options, working_dir, running)?;
    } else {
        println!("\nOperation cancelled");
        std::process::exit(0);
    }
    Ok(())
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

    if matches!(operation, Operation::Apply) {
        command.arg("-auto-approve");
    }

    let command_str = format!(
        "{} {} {}",
        terraform_binary,
        operation,
        target_options.join(" "),
    );
    let command_str = if matches!(operation, Operation::Apply) {
        format!("{} -auto-approve", command_str)
    } else {
        command_str
    };

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
}
