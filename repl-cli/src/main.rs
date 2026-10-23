fn parse_input(input: &str) -> Result<&str, &'static str> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err("command cannot be empty");
    }

    Ok(trimmed)
}

fn main() {
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "status".to_string());

    match parse_input(&command) {
        Ok(command) => {
            println!("KARIS-KY REPL CLI");
            println!("Command received: {command}");
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_input;

    #[test]
    fn parse_input_accepts_non_empty_commands() {
        assert_eq!(parse_input("  status  ").unwrap(), "status");
    }

    #[test]
    fn parse_input_rejects_empty_input() {
        assert!(parse_input("   ").is_err());
    }
}
