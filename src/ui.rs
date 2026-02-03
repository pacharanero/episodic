use anyhow::{anyhow, Result};
use chrono::{Local, NaiveDate};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
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

/// Get the proposed date, prompting if not provided (defaults to today).
/// Returns the ISO string and parsed NaiveDate.
pub fn ask_date(date: Option<String>) -> Result<(String, NaiveDate)> {
    let today = Local::now().date_naive();
    let today_iso = today.format("%Y-%m-%d").to_string();

    let input = match date {
        Some(val) => val,
        None => Text::new("Proposed Date (YYYY-MM-DD):")
            .with_default(&today_iso)
            .prompt()?,
    };

    if let Ok(parsed) = NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
        Ok((input, parsed))
    } else {
        Err(anyhow!(
            "Invalid date format '{}'. Please use YYYY-MM-DD.",
            input
        ))
    }
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
