//! Implementation of the `edh new` command.

use anyhow::Result;
use console::style;
use inquire::Confirm;

use crate::config::{extract_google_id, Config};
use crate::formatting;
use crate::google::GoogleBackend;
use crate::ui::{ask_date, ask_title, print_banner, with_spinner};

/// Create all resources for a new EDH episode.
pub async fn new_episode(title: Option<String>, date: Option<String>) -> Result<()> {
    print_banner();

    let mut config = Config::load_from_env_file("edh-live.env")?;

    let episode_title = ask_title(title)?;
    let (proposed_date_iso, proposed_date) = ask_date(date)?;
    let kebab = formatting::kebab_case(&episode_title);
    let pretty = formatting::pretty_date(&proposed_date);
    let folder_name = format!("{}-{}", pretty, kebab);

    println!(
        "{} {}",
        style("Episode Title:").cyan(),
        style(&episode_title).cyan()
    );
    println!(
        "{} {}",
        style("Date:").cyan(),
        style(format!("{} -> {}", proposed_date_iso, pretty)).cyan()
    );
    println!(
        "{} {}",
        style("Folder name:").cyan(),
        style(&folder_name).cyan()
    );

    // Fetch URLs/IDs from config (supports both full URLs and plain IDs)
    let planning_folder_url = config.fetch_required("EDH_IN_PLANNING_GDRIVE_FOLDER_URL")?;
    let template_doc_url = config.fetch_required("EDH_EPISODE_GUIDE_TEMPLATE_GDOC_URL")?;
    let planned_doc_url = config.fetch_required("START_HERE_EDH_GDOC_URL")?;
    let calendar_id = config.fetch_optional("EDH_CALENDAR_ID", "primary");

    // Extract IDs from URLs for API calls
    let planning_folder_id = extract_google_id(&planning_folder_url);
    let template_doc_id = extract_google_id(&template_doc_url);
    let planned_doc_id = extract_google_id(&planned_doc_url);

    let backend = GoogleBackend::new().await?;

    let mut folder = None;
    let mut doc = None;

    // Step 1: Create Drive folder
    if Confirm::new(&format!(
        "Create a new folder named '{}' in the planning folder?",
        folder_name
    ))
    .with_default(false)
    .prompt()?
    {
        folder = with_spinner(
            "Creating Drive folder...",
            backend.create_episode_folder(&folder_name, &planning_folder_id),
        )
        .await?;
        if let Some(folder_val) = folder.as_ref() {
            println!(
                "{} {}",
                style("Folder created:").green(),
                style(&folder_val.url).green()
            );
        }
    }

    // Step 2: Copy Episode Guide template
    if let Some(folder_val) = folder.as_ref() {
        if Confirm::new("Copy the Episode Guide template into that folder?")
            .with_default(false)
            .prompt()?
        {
            doc = with_spinner(
                "Copying Episode Guide template...",
                backend.copy_template_to_folder(&template_doc_id, &folder_val.id, &episode_title),
            )
            .await?;
            if let Some(doc_val) = doc.as_ref() {
                println!(
                    "{} {}",
                    style("Episode guide:").green(),
                    style(&doc_val.url).green()
                );
                if open::that(&doc_val.url).is_err() {
                    println!(
                        "{} {}",
                        style("Open manually:").yellow(),
                        style(&doc_val.url).yellow()
                    );
                }
            }
        }
    }

    // Step 3: Update Episode Guide document
    if let (Some(doc_val), Some(folder_val)) = (doc.as_ref(), folder.as_ref()) {
        if Confirm::new("Make Episode Guide document changes?")
            .with_default(false)
            .prompt()?
        {
            with_spinner(
                "Updating Episode Guide document...",
                backend.update_episode_doc(
                    &doc_val.id,
                    &episode_title,
                    &proposed_date_iso,
                    &folder_val.url,
                ),
            )
            .await?;
        }
    } else if doc.is_some() && folder.is_none() {
        println!(
            "{}",
            style("Skipping doc updates because no folder URL is available.").yellow()
        );
    }

    // Step 4: Create Calendar event
    if Confirm::new("Add an all-day event to Google Calendar with the episode title/date?")
        .with_default(false)
        .prompt()?
    {
        let event_link = with_spinner(
            "Creating Calendar event...",
            backend.add_calendar_event(&calendar_id, &episode_title, &proposed_date_iso),
        )
        .await?;
        if let Some(link) = event_link {
            println!(
                "{} {}",
                style("Calendar event created:").green(),
                style(link).green()
            );
        }
    }

    // Step 5: Append to Planned Episodes doc
    if folder.is_some()
        && Confirm::new("Append the episode to the Planned Episodes doc?")
            .with_default(false)
            .prompt()?
    {
        with_spinner(
            "Appending to Planned Episodes doc...",
            backend.append_planned_episode_entry(
                &planned_doc_id,
                &episode_title,
                &proposed_date_iso,
                &folder.as_ref().unwrap().url,
            ),
        )
        .await?;

        // Use the configured URL if it looks like a URL, otherwise construct one
        let open_url = if planned_doc_url.starts_with("http") {
            planned_doc_url.clone()
        } else {
            format!("https://docs.google.com/document/d/{}/edit", planned_doc_id)
        };
        if open::that(&open_url).is_err() {
            println!(
                "{} {}",
                style("Open Planned Episodes doc manually:").yellow(),
                style(&open_url).yellow(),
            );
        }
    } else if folder.is_none()
        && Confirm::new("Append the episode to the Planned Episodes doc?")
            .with_default(false)
            .prompt()?
    {
        println!(
            "{}",
            style("Skipping append because no folder URL is available.").yellow()
        );
    }

    println!("{}", style("Done.").green());
    Ok(())
}
