use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::validator::Validation;
use inquire::Text;

/// Print the CLI banner.
pub fn print_banner() {
    let title = style("EVERYTHING DIGITAL HEALTH").bold().cyan();
    let subtitle = style("episode setup").dim();
    println!("{} {}", title, subtitle);
    println!(
        "{}",
        style("────────────────────────────────────────").dim()
    );
}

/// Prompt for episode title or use provided value.
pub fn ask_title(title: Option<String>) -> Result<String> {
    let input = match title {
        Some(val) => val,
        None => Text::new("Episode Title (Title Case):").prompt()?,
    };
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Episode title is required."));
    }
    Ok(trimmed.to_string())
}

/// Prompt for proposed date or use provided value.
/// Returns the ISO string and parsed NaiveDate.
pub fn ask_date(date: Option<String>) -> Result<(String, NaiveDate)> {
    let mut input = date;
    loop {
        let value = match input.take() {
            Some(val) => val,
            None => Text::new("Proposed Date (ISO 8601 YYYY-MM-DD):")
                .with_validator(|val: &str| {
                    if is_iso_date(val) {
                        Ok(Validation::Valid)
                    } else {
                        Ok(Validation::Invalid("Use YYYY-MM-DD".into()))
                    }
                })
                .prompt()?,
        };

        if let Ok(date) = NaiveDate::parse_from_str(&value, "%Y-%m-%d") {
            return Ok((value, date));
        }
        println!(
            "{}",
            style("Invalid date. Please use a real calendar date (YYYY-MM-DD).").red()
        );
    }
}

/// Check if a string matches YYYY-MM-DD format (syntax only, not validity).
fn is_iso_date(val: &str) -> bool {
    let bytes = val.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[8..10].iter().all(|b| b.is_ascii_digit())
}

/// Run an async operation with a spinner, showing success/failure on completion.
pub async fn with_spinner<T>(
    label: &str,
    work: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message(label.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = work.await;
    match &result {
        Ok(_) => spinner.finish_with_message(format!("{} {}", label, style("done").green())),
        Err(_) => spinner.finish_with_message(format!("{} {}", label, style("failed").red())),
    }
    result
}
