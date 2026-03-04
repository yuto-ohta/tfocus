use crate::cli::Operation;
use crate::error::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal;

pub struct InputHandler;

enum OperationInputAction {
    Select(Operation),
    Cancel,
    Ignore,
}

struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

impl InputHandler {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn read_operation(&mut self) -> Result<Operation> {
        terminal::enable_raw_mode()?;
        let raw_mode_guard = RawModeGuard;

        loop {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match parse_operation_key(key) {
                    OperationInputAction::Select(operation) => return Ok(operation),
                    OperationInputAction::Cancel => {
                        drop(raw_mode_guard);
                        println!("\nOperation cancelled by user");
                        std::process::exit(0);
                    }
                    OperationInputAction::Ignore => {}
                }
            }
        }
    }
}

fn parse_operation_key(key: KeyEvent) -> OperationInputAction {
    match key.code {
        KeyCode::Esc => OperationInputAction::Cancel,
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'c') {
                return OperationInputAction::Cancel;
            }

            let is_plain_input =
                key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT;

            if !is_plain_input {
                return OperationInputAction::Ignore;
            }

            match c.to_ascii_lowercase() {
                'p' => OperationInputAction::Select(Operation::Plan),
                'a' => OperationInputAction::Select(Operation::Apply),
                _ => OperationInputAction::Ignore,
            }
        }
        _ => OperationInputAction::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_operation_key_plan_lowercase() {
        let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(matches!(
            parse_operation_key(key),
            OperationInputAction::Select(Operation::Plan)
        ));
    }

    #[test]
    fn test_parse_operation_key_plan_uppercase() {
        let key = KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT);
        assert!(matches!(
            parse_operation_key(key),
            OperationInputAction::Select(Operation::Plan)
        ));
    }

    #[test]
    fn test_parse_operation_key_apply_lowercase() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(matches!(
            parse_operation_key(key),
            OperationInputAction::Select(Operation::Apply)
        ));
    }

    #[test]
    fn test_parse_operation_key_apply_uppercase() {
        let key = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(matches!(
            parse_operation_key(key),
            OperationInputAction::Select(Operation::Apply)
        ));
    }

    #[test]
    fn test_parse_operation_key_ctrl_c_cancels() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(
            parse_operation_key(key),
            OperationInputAction::Cancel
        ));
    }

    #[test]
    fn test_parse_operation_key_escape_cancels() {
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(matches!(
            parse_operation_key(key),
            OperationInputAction::Cancel
        ));
    }

    #[test]
    fn test_parse_operation_key_number_is_ignored() {
        let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
        assert!(matches!(
            parse_operation_key(key),
            OperationInputAction::Ignore
        ));
    }

    #[test]
    fn test_parse_operation_key_other_key_is_ignored() {
        let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert!(matches!(
            parse_operation_key(key),
            OperationInputAction::Ignore
        ));
    }
}
