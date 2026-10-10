use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use urlencoding::encode;
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};

use super::types::*;

const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/drive",
    "https://www.googleapis.com/auth/documents",
    "https://www.googleapis.com/auth/calendar",
];

/// Google API backend using OAuth2 authentication.
#[derive(Clone)]
pub struct GoogleBackend {
    http: reqwest::Client,
    token: String,
}

impl GoogleBackend {
    /// Create a new backend using OAuth2 authentication.
    pub async fn new() -> Result<Self> {
        let secret_path = Self::find_client_secret()?;
        let secret = yup_oauth2::read_application_secret(&secret_path)
            .await
            .with_context(|| {
                format!("Failed to read OAuth client secret from {:?}", secret_path)
            })?;

        let token_cache = Self::token_cache_path();
        if let Some(parent) = token_cache.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let auth =
            InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
                .persist_tokens_to_disk(&token_cache)
                .build()
                .await
                .context("Failed to build OAuth authenticator")?;

        let token = auth
            .token(SCOPES)
            .await
            .context("Failed to obtain Google access token")?;

        let access_token = token
            .token()
            .map(|t| t.to_string())
            .ok_or_else(|| anyhow!("OAuth token response did not include an access token"))?;

        Ok(Self {
            http: reqwest::Client::new(),
            token: access_token,
        })
    }

    /// Find the client_secret.json file for OAuth.
    fn find_client_secret() -> Result<PathBuf> {
        // Check common locations in order of preference
        let mut candidates = vec![
            PathBuf::from("client_secret.json"),
            PathBuf::from(".secrets/client_secret.json"),
        ];

        // Also check for any client_secret*.json in .secrets/
        if let Ok(entries) = std::fs::read_dir(".secrets") {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains("client_secret") && name.ends_with(".json") {
                        candidates.push(path);
                    }
                }
            }
        }

        candidates.extend([
            dirs::config_dir()
                .map(|d| d.join("edh").join("client_secret.json"))
                .unwrap_or_default(),
            dirs::home_dir()
                .map(|d| d.join(".edh").join("client_secret.json"))
                .unwrap_or_default(),
        ]);

        for path in &candidates {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        Err(anyhow!(
            "No Google OAuth client secret found. Please download OAuth 2.0 credentials from \
             Google Cloud Console and save as one of:\n  \
             - ./client_secret.json\n  \
             - ./.secrets/client_secret.json\n  \
             - ~/.edh/client_secret.json"
        ))
    }

    /// Path for caching OAuth tokens between runs.
    fn token_cache_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".edh")
            .join("token_cache.json")
    }

    /// Create a new folder in Google Drive.
    pub async fn create_episode_folder(
        &self,
        name: &str,
        parent_id: &str,
    ) -> Result<Option<DriveFolder>> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            parents: [&'a str; 1],
            #[serde(rename = "mimeType")]
            mime_type: &'a str,
        }

        #[derive(Deserialize)]
        struct Resp {
            id: String,
            #[serde(rename = "webViewLink")]
            web_view_link: Option<String>,
        }

        let url = "https://www.googleapis.com/drive/v3/files?fields=id,webViewLink";
        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(&Body {
                name,
                parents: [parent_id],
                mime_type: "application/vnd.google-apps.folder",
            })
            .send()
            .await
            .context("Drive create folder request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Drive create folder failed ({}): {}", status, text));
        }
        let data: Resp = resp
            .json()
            .await
            .context("Failed to parse Drive create folder response")?;
        let url = data
            .web_view_link
            .unwrap_or_else(|| format!("https://drive.google.com/drive/folders/{}", data.id));
        Ok(Some(DriveFolder { id: data.id, url }))
    }

    /// Copy a Google Doc template into a folder with a new title.
    pub async fn copy_template_to_folder(
        &self,
        template_id: &str,
        folder_id: &str,
        new_title: &str,
    ) -> Result<Option<DriveDoc>> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            parents: [&'a str; 1],
        }

        #[derive(Deserialize)]
        struct Resp {
            id: String,
            #[serde(rename = "webViewLink")]
            web_view_link: Option<String>,
        }

        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}/copy?fields=id,webViewLink",
            encode(template_id)
        );
        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(&Body {
                name: new_title,
                parents: [folder_id],
            })
            .send()
            .await
            .context("Drive copy template request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Drive copy failed ({}): {}", status, text));
        }

        let data: Resp = resp
            .json()
            .await
            .context("Failed to parse Drive copy response")?;
        let url = data
            .web_view_link
            .unwrap_or_else(|| format!("https://docs.google.com/document/d/{}/edit", data.id));
        Ok(Some(DriveDoc { id: data.id, url }))
    }

    /// Update an episode guide document with title, date, and folder URL.
    pub async fn update_episode_doc(
        &self,
        doc_id: &str,
        episode_title: &str,
        proposed_date_iso: &str,
        folder_url: &str,
    ) -> Result<()> {
        let doc_url = format!(
            "https://docs.googleapis.com/v1/documents/{}",
            encode(doc_id)
        );
        let resp = self
            .http
            .get(doc_url)
            .bearer_auth(&self.token)
            .send()
            .await
            .context("Docs get document request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Docs get document failed ({}): {}", status, text));
        }

        let doc: DocsDocument = resp.json().await.context("Failed to parse Docs document")?;
        let mut requests: Vec<DocsRequest> = Vec::new();

        let heading = doc
            .body
            .as_ref()
            .and_then(|b| b.content.as_ref())
            .and_then(|content| {
                content.iter().find(|el| {
                    el.paragraph
                        .as_ref()
                        .and_then(|p| p.paragraph_style.as_ref())
                        .and_then(|s| s.named_style_type.as_deref())
                        == Some("HEADING_1")
                })
            });

        if let Some(h) = heading {
            if let (Some(start), Some(end)) = (h.start_index, h.end_index) {
                if end > start + 1 {
                    requests.push(DocsRequest::DeleteContentRange {
                        delete_content_range: DocsDeleteContentRange {
                            range: DocsRange {
                                start_index: start,
                                end_index: end - 1,
                            },
                        },
                    });
                }

                requests.push(DocsRequest::InsertText {
                    insert_text: DocsInsertText {
                        location: DocsLocation { index: start },
                        text: episode_title.to_string(),
                    },
                });
            }
        }

        let insert_index = heading.and_then(|h| h.end_index).unwrap_or(1);
        let info_line = format!(
            "\nProposed Date: {}\nEpisode Folder: {}\n\n",
            proposed_date_iso, folder_url
        );
        requests.push(DocsRequest::InsertText {
            insert_text: DocsInsertText {
                location: DocsLocation {
                    index: insert_index,
                },
                text: info_line,
            },
        });

        if requests.is_empty() {
            return Ok(());
        }

        let batch_url = format!(
            "https://docs.googleapis.com/v1/documents/{}:batchUpdate",
            encode(doc_id)
        );
        let resp = self
            .http
            .post(batch_url)
            .bearer_auth(&self.token)
            .json(&DocsBatchUpdateRequest { requests })
            .send()
            .await
            .context("Docs batchUpdate request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Docs batchUpdate failed ({}): {}", status, text));
        }
        Ok(())
    }

    /// Create an all-day calendar event.
    pub async fn add_calendar_event(
        &self,
        calendar_id: &str,
        episode_title: &str,
        proposed_date_iso: &str,
    ) -> Result<Option<String>> {
        #[derive(Serialize)]
        struct DateObj<'a> {
            date: &'a str,
        }

        #[derive(Serialize)]
        struct Body<'a> {
            summary: &'a str,
            start: DateObj<'a>,
            end: DateObj<'a>,
        }

        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "htmlLink")]
            html_link: Option<String>,
        }

        let date = NaiveDate::parse_from_str(proposed_date_iso, "%Y-%m-%d")
            .context("Invalid proposed date while creating calendar event")?;
        let end_date = date.succ_opt().ok_or_else(|| anyhow!("Invalid end date"))?;
        let end_date_iso = end_date.format("%Y-%m-%d").to_string();

        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events",
            encode(calendar_id)
        );

        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(&Body {
                summary: episode_title,
                start: DateObj {
                    date: proposed_date_iso,
                },
                end: DateObj {
                    date: &end_date_iso,
                },
            })
            .send()
            .await
            .context("Calendar insert event request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Calendar insert event failed ({}): {}",
                status,
                text
            ));
        }
        let data: Resp = resp
            .json()
            .await
            .context("Failed to parse Calendar insert response")?;
        Ok(data.html_link)
    }

    /// Append an episode entry to the planned episodes document.
    pub async fn append_planned_episode_entry(
        &self,
        planned_doc_id: &str,
        episode_title: &str,
        proposed_date_iso: &str,
        folder_url: &str,
    ) -> Result<()> {
        let line = format!(
            "{} - {} - {}\n",
            proposed_date_iso, episode_title, folder_url
        );
        let requests = vec![DocsRequest::InsertTextEnd {
            insert_text: DocsInsertTextEnd {
                end_of_segment_location: serde_json::json!({}),
                text: line,
            },
        }];

        let url = format!(
            "https://docs.googleapis.com/v1/documents/{}:batchUpdate",
            encode(planned_doc_id)
        );
        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(&DocsBatchUpdateRequest { requests })
            .send()
            .await
            .context("Planned Episodes batchUpdate request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Planned Episodes update failed ({}): {}",
                status,
                text
            ));
        }
        Ok(())
    }
}
