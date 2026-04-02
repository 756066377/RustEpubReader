use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InlineStyle {
    Normal,
    Bold,
    Italic,
    BoldItalic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextSpan {
    pub text: String,
    pub style: InlineStyle,
    pub link_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    Heading {
        level: u8,
        spans: Vec<TextSpan>,
    },
    Paragraph {
        spans: Vec<TextSpan>,
    },
    Image {
        data: Arc<Vec<u8>>,
        alt: Option<String>,
    },
    Separator,
    BlankLine,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub blocks: Vec<ContentBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_href: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TocEntry {
    pub title: String,
    pub chapter_index: usize,
}
