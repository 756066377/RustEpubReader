//! Core EPUB parsing, rendering, and book processing logic.
pub mod cache;
pub mod chapter;
pub mod parser;

pub use chapter::{
    Chapter, ContentBlock, CorrectionInfo, CorrectionStatus, InlineStyle, TextSpan, TocEntry,
};
pub use parser::{EpubBook, EpubMetadata};
