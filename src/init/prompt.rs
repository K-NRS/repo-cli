use std::io::{self, Write};

use anyhow::Result;
use colored::Colorize;

use super::detect::ProjectType;

/// Display project type selection menu and get user choice
pub fn prompt_project_type(detected: Option<ProjectType>) -> Result<ProjectType> {
    let types = ProjectType::all();

    println!();
    println!("{}", "Select project type:".bold());
    println!();

    for (i, pt) in types.iter().enumerate() {
        let num = if i == 9 { 0 } else { i + 1 };
        let is_detected = detected == Some(*pt);

        if is_detected {
            println!(
                "  [{}] {} {}",
                num.to_string().cyan(),
                pt.name().cyan().bold(),
                "(detected)".green()
            );
        } else {
            println!("  [{}] {}", num.to_string().dimmed(), pt.name());
        }
    }

    println!();

    loop {
        if let Some(det) = detected {
            print!(
                "{} Select [1-9, 0] or press Enter for {}: ",
                "?".yellow().bold(),
                det.name().cyan()
            );
        } else {
            print!("{} Select [1-9, 0]: ", "?".yellow().bold());
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        // Enter key with detection = use detected
        if input.is_empty() {
            if let Some(det) = detected {
                return Ok(det);
            }
            println!("  {}", "Please select a number".dimmed());
            continue;
        }

        // Parse number
        match input.parse::<usize>() {
            Ok(0) => return Ok(types[9]), // Generic
            Ok(n) if n >= 1 && n <= 9 => return Ok(types[n - 1]),
            _ => {
                println!("  {}", "Invalid selection. Enter 1-9 or 0".dimmed());
            }
        }
    }
}

/// Confirm overwriting existing workflow
pub fn prompt_overwrite() -> Result<bool> {
    print!(
        "{} Workflow already exists. Overwrite? [y/N] ",
        "?".yellow().bold()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_lowercase() == "y")
}
