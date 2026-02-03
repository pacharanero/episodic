use serde::{Deserialize, Serialize};

/// A Google Drive folder reference.
#[derive(Clone, Debug)]
pub struct DriveFolder {
    pub id: String,
    pub url: String,
}

/// A Google Drive document reference.
#[derive(Clone, Debug)]
pub struct DriveDoc {
    pub id: String,
    pub url: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Google Docs API types (for reading/updating documents)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct DocsDocument {
    pub body: Option<DocsBody>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DocsBody {
    pub content: Option<Vec<DocsStructuralElement>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DocsStructuralElement {
    #[serde(rename = "startIndex")]
    pub start_index: Option<i32>,
    #[serde(rename = "endIndex")]
    pub end_index: Option<i32>,
    pub paragraph: Option<DocsParagraph>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DocsParagraph {
    #[serde(rename = "paragraphStyle")]
    pub paragraph_style: Option<DocsParagraphStyle>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DocsParagraphStyle {
    #[serde(rename = "namedStyleType")]
    pub named_style_type: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Google Docs API request types (for batchUpdate)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct DocsBatchUpdateRequest {
    pub requests: Vec<DocsRequest>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum DocsRequest {
    #[serde(rename_all = "camelCase")]
    DeleteContentRange {
        delete_content_range: DocsDeleteContentRange,
    },
    #[serde(rename_all = "camelCase")]
    InsertText { insert_text: DocsInsertText },
    #[serde(rename_all = "camelCase")]
    InsertTextEnd { insert_text: DocsInsertTextEnd },
}

#[derive(Debug, Serialize)]
pub(crate) struct DocsDeleteContentRange {
    pub range: DocsRange,
}

#[derive(Debug, Serialize)]
pub(crate) struct DocsRange {
    #[serde(rename = "startIndex")]
    pub start_index: i32,
    #[serde(rename = "endIndex")]
    pub end_index: i32,
}

#[derive(Debug, Serialize)]
pub(crate) struct DocsInsertText {
    pub location: DocsLocation,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DocsLocation {
    pub index: i32,
}

#[derive(Debug, Serialize)]
pub(crate) struct DocsInsertTextEnd {
    #[serde(rename = "endOfSegmentLocation")]
    pub end_of_segment_location: serde_json::Value,
    pub text: String,
}
