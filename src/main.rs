mod cli;
mod display;
mod error;
mod executor;
mod input;
mod project;
mod selector;
mod types;

use clap::Parser;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use crate::cli::{Cli, Operation, SelectionType};
use crate::display::Display;
use crate::error::{Result, TfocusError};
use crate::input::InputHandler;
use crate::project::TerraformProject;
use crate::selector::{SelectItem, Selector};
use crate::types::{Resource, Target};

#[derive(Debug)]
enum SelectionItem {
    File(usize, std::path::PathBuf),
    Module(usize, String),
    Resource(usize, Resource),
}

fn create_selection_items(selection_items: &[SelectionItem]) -> Vec<SelectItem> {
    selection_items
        .iter()
        .map(|item| {
            let (display, search_text) = match item {
                SelectionItem::File(idx, path) => {
                    let path_str = path.display().to_string();
                    (
                        format!("{:4} {:15} {}", idx, "[File]", path_str),
                        path_str.clone(),
                    )
                }
                SelectionItem::Module(idx, name) => (
                    format!("{:4} {:15} {}", idx, "[Module]", name),
                    name.clone(),
                ),
                SelectionItem::Resource(idx, resource) => {
                    let resource_str = if resource.is_module {
                        format!("module.{}", resource.name)
                    } else {
                        format!("{}.{}", resource.resource_type, resource.name)
                    };
                    (
                        format!(
                            "{:4} {:15} {}",
                            idx,
                            if resource.is_module {
                                "[Module]"
                            } else {
                                "[Resource]"
                            },
                            resource_str
                        ),
                        resource_str,
                    )
                }
            };
            SelectItem {
                display,
                search_text,
                data: match item {
                    SelectionItem::File(_, path) => {
                        format!("f:{}", path.display())
                    }
                    SelectionItem::Module(_, name) => {
                        format!("m:{}", name)
                    }
                    SelectionItem::Resource(_, resource) => {
                        if resource.is_module {
                            format!("m:{}", resource.name)
                        } else {
                            format!("r:{}.{}", resource.resource_type, resource.name)
                        }
                    }
                },
            }
        })
        .collect()
}

fn parse_selected_item(selected: &str) -> Result<Target> {
    if let Some(stripped) = selected.strip_prefix("f:") {
        let path = Path::new(stripped).to_path_buf();
        Ok(Target::File(path))
    } else if let Some(stripped) = selected.strip_prefix("m:") {
        Ok(Target::Module(stripped.to_string()))
    } else if let Some(stripped) = selected.strip_prefix("r:") {
        let (resource_type, name) = stripped
            .split_once('.')
            .ok_or(TfocusError::InvalidTargetSelection)?;
        Ok(Target::Resource(
            resource_type.to_string(),
            name.to_string(),
        ))
    } else {
        Err(TfocusError::InvalidTargetSelection)
    }
}

fn parse_selected_items(selected_items: &[String]) -> Result<Vec<Target>> {
    selected_items
        .iter()
        .map(|item| parse_selected_item(item))
        .collect()
}

fn collect_resources_for_target(project: &TerraformProject, target: &Target) -> Vec<Resource> {
    match target {
        Target::File(path) => project.get_resources_by_target(&Target::File(path.clone())),
        Target::Module(name) => project.get_resources_by_target(&Target::Module(name.clone())),
        Target::Resource(resource_type, name) => {
            project.get_resources_by_target(&Target::Resource(resource_type.clone(), name.clone()))
        }
    }
}

fn deduplicate_resources(resources: Vec<Resource>) -> Vec<Resource> {
    let mut seen_targets = HashSet::new();
    let mut deduplicated = Vec::new();

    for resource in resources {
        let target = resource.target_string();
        if seen_targets.insert(target) {
            deduplicated.push(resource);
        }
    }

    deduplicated
}

fn collect_selected_resources(project: &TerraformProject, targets: &[Target]) -> Vec<Resource> {
    targets
        .iter()
        .flat_map(|target| collect_resources_for_target(project, target))
        .collect()
}

fn validate_single_working_directory(resources: &[Resource]) -> Result<()> {
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

    if unique_dirs.len() <= 1 {
        return Ok(());
    }

    Err(TfocusError::MixedWorkingDirectories(
        unique_dirs.into_iter().collect(),
    ))
}

fn prompt_operation_selection() -> Result<Operation> {
    Display::print_header("Select operation:");
    println!("  p) plan");
    println!("  a) apply");
    println!("  Press p/a to confirm, Esc/Ctrl+C to cancel");

    let mut input = InputHandler::new()?;
    input.read_operation()
}

fn resolve_operation_with_prompt<F>(
    operation: Option<Operation>,
    non_interactive: bool,
    mut prompt: F,
) -> Result<Operation>
where
    F: FnMut() -> Result<Operation>,
{
    if let Some(operation) = operation {
        return Ok(operation);
    }

    if non_interactive {
        return Err(TfocusError::ParseError(
            "In --non-interactive mode, --operation must be specified (plan or apply)".to_string(),
        ));
    }

    prompt()
}

fn main() -> Result<()> {
    // setting env
    env_logger::init();
    let cli = Cli::parse();

    if cli.verbose {
        std::env::set_var("RUST_LOG", "debug");
    }

    // Parse the Terraform project
    let project = match TerraformProject::parse_directory(Path::new(&cli.path)) {
        Ok(project) => project,
        Err(TfocusError::NoTerraformFiles) => {
            eprintln!("Error: No Terraform files found in the current directory or its children.");
            eprintln!("Please run this command from a directory containing Terraform files.");
            std::process::exit(1);
        }
        Err(e) => return Err(e),
    };

    // Collect all targets
    let mut selection_items = Vec::new();
    let mut current_index = 1;

    let stype = cli.selection_type;

    // add files
    if matches!(stype, None | Some(SelectionType::File)) {
        for file in project.get_unique_files() {
            selection_items.push(SelectionItem::File(current_index, file));
            current_index += 1;
        }
    }

    // add modules
    if matches!(stype, None | Some(SelectionType::Module)) {
        for module in project.get_modules() {
            selection_items.push(SelectionItem::Module(current_index, module));
            current_index += 1;
        }
    }

    // add resources (always shown)
    for resource in project.get_all_resources() {
        selection_items.push(SelectionItem::Resource(current_index, resource));
        current_index += 1;
    }

    // Initialize and run the selector
    let selector_items = create_selection_items(&selection_items);
    let mut selector = Selector::new(selector_items);

    let selected_items = match selector.run()? {
        Some(data) if !data.is_empty() => data,
        None => {
            println!("\nOperation cancelled");
            std::process::exit(0);
        }
        _ => {
            println!("\nNo items selected");
            std::process::exit(0);
        }
    };

    let targets = parse_selected_items(&selected_items)?;

    // Get the resources for the selected targets
    let selected_resources = collect_selected_resources(&project, &targets);

    if selected_resources.is_empty() {
        println!("\nNo resources found for the selected targets.");
        return Ok(());
    }

    validate_single_working_directory(&selected_resources)?;
    let resources = deduplicate_resources(selected_resources);

    Display::print_header("\nSelected resources:");
    for resource in &resources {
        Display::print_resource(resource);
    }

    println!();
    let operation = resolve_operation_with_prompt(cli.operation, cli.non_interactive, || {
        prompt_operation_selection()
    })?;

    println!();
    // Execute the selected resources
    executor::execute_with_resources(&resources, operation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_resource(target_name: &str, file_path: &str) -> Resource {
        Resource {
            resource_type: "aws_instance".to_string(),
            name: target_name.to_string(),
            is_module: false,
            file_path: PathBuf::from(file_path),
            has_count: false,
            has_for_each: false,
            index: None,
        }
    }

    #[test]
    fn test_parse_selected_item_file() {
        let target = parse_selected_item("f:/tmp/main.tf").unwrap();
        assert_eq!(target, Target::File(PathBuf::from("/tmp/main.tf")));
    }

    #[test]
    fn test_parse_selected_item_module() {
        let target = parse_selected_item("m:vpc").unwrap();
        assert_eq!(target, Target::Module("vpc".to_string()));
    }

    #[test]
    fn test_parse_selected_item_resource() {
        let target = parse_selected_item("r:aws_instance.web").unwrap();
        assert_eq!(
            target,
            Target::Resource("aws_instance".to_string(), "web".to_string())
        );
    }

    #[test]
    fn test_parse_selected_item_invalid_resource() {
        let result = parse_selected_item("r:aws_instance");
        assert!(matches!(result, Err(TfocusError::InvalidTargetSelection)));
    }

    #[test]
    fn test_deduplicate_resources() {
        let resources = vec![
            create_resource("web", "/tmp/main.tf"),
            create_resource("web", "/tmp/main.tf"),
            create_resource("app", "/tmp/main.tf"),
        ];

        let deduplicated = deduplicate_resources(resources);
        assert_eq!(deduplicated.len(), 2);
        assert_eq!(deduplicated[0].target_string(), "aws_instance.web");
        assert_eq!(deduplicated[1].target_string(), "aws_instance.app");
    }

    #[test]
    fn test_validate_single_working_directory_same_directory() {
        let resources = vec![
            create_resource("web", "/tmp/a/main.tf"),
            create_resource("app", "/tmp/a/network.tf"),
        ];

        assert!(validate_single_working_directory(&resources).is_ok());
    }

    #[test]
    fn test_validate_single_working_directory_mixed_directories() {
        let resources = vec![
            create_resource("web", "/tmp/a/main.tf"),
            create_resource("app", "/tmp/b/network.tf"),
        ];

        let result = validate_single_working_directory(&resources);
        assert!(matches!(
            result,
            Err(TfocusError::MixedWorkingDirectories(_))
        ));
    }

    #[test]
    fn test_validate_single_working_directory_mixed_directories_with_same_target() {
        let resources = vec![
            create_resource("web", "/tmp/a/main.tf"),
            create_resource("web", "/tmp/b/network.tf"),
        ];

        let result = validate_single_working_directory(&resources);
        assert!(matches!(
            result,
            Err(TfocusError::MixedWorkingDirectories(_))
        ));
    }

    #[test]
    fn test_resolve_operation_prefers_cli_option_without_prompt() {
        let mut prompted = false;
        let operation = resolve_operation_with_prompt(Some(Operation::Apply), false, || {
            prompted = true;
            Ok(Operation::Plan)
        })
        .unwrap();

        assert!(matches!(operation, Operation::Apply));
        assert!(!prompted);
    }

    #[test]
    fn test_resolve_operation_requires_operation_in_non_interactive_mode() {
        let result = resolve_operation_with_prompt(None, true, || Ok(Operation::Plan));

        assert!(
            matches!(result, Err(TfocusError::ParseError(message)) if message.contains("--operation must be specified"))
        );
    }

    #[test]
    fn test_resolve_operation_prompts_when_interactive_and_no_cli_option() {
        let operation = resolve_operation_with_prompt(None, false, || Ok(Operation::Plan)).unwrap();

        assert!(matches!(operation, Operation::Plan));
    }
}
