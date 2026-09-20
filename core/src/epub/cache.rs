//! Parsed-book cache so reopening a large EPUB does not re-parse HTML.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::chapter::{Chapter, TocEntry};
use super::parser::EpubBook;

const CACHE_MAGIC: &[u8; 8] = b"RERPC001";

#[derive(serde::Serialize, serde::Deserialize)]
struct ParseCache {
    title: String,
    chapters: Vec<Chapter>,
    toc: Vec<TocEntry>,
    cover_data: Option<Vec<u8>>,
    fonts: Vec<(String, Vec<u8>)>,
    chapter_reviews: std::collections::HashMap<usize, usize>,
    review_chapter_indices: std::collections::HashSet<usize>,
}

fn epub_mtime_size(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some((mtime, meta.len()))
}

pub fn sibling_cache_path(epub_path: &Path) -> PathBuf {
    epub_path.with_extension("parse.bin")
}

impl EpubBook {
    pub fn load_parse_cache(epub_path: &Path) -> Option<Self> {
        let (mtime, size) = epub_mtime_size(epub_path)?;
        let cache_path = sibling_cache_path(epub_path);
        let data = fs::read(&cache_path).ok()?;
        if data.len() < 24 || data.get(..8)? != CACHE_MAGIC {
            return None;
        }
        let stored_mtime = u64::from_le_bytes(data[8..16].try_into().ok()?);
        let stored_size = u64::from_le_bytes(data[16..24].try_into().ok()?);
        if stored_mtime != mtime || stored_size != size {
            return None;
        }
        let cached: ParseCache = bincode::deserialize(&data[24..]).ok()?;
        Some(Self {
            title: cached.title,
            chapters: cached.chapters,
            toc: cached.toc,
            cover_data: cached.cover_data,
            fonts: cached.fonts,
            chapter_reviews: cached.chapter_reviews,
            review_chapter_indices: cached.review_chapter_indices,
        })
    }

    pub fn save_parse_cache(epub_path: &Path, book: &Self) -> Result<PathBuf, String> {
        let (mtime, size) =
            epub_mtime_size(epub_path).ok_or_else(|| "无法读取 EPUB 文件信息".to_string())?;
        let cache_path = sibling_cache_path(epub_path);
        let cached = ParseCache {
            title: book.title.clone(),
            chapters: book.chapters.clone(),
            toc: book.toc.clone(),
            cover_data: book.cover_data.clone(),
            fonts: book.fonts.clone(),
            chapter_reviews: book.chapter_reviews.clone(),
            review_chapter_indices: book.review_chapter_indices.clone(),
        };
        let payload = bincode::serialize(&cached).map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(24 + payload.len());
        out.extend_from_slice(CACHE_MAGIC);
        out.extend_from_slice(&mtime.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&payload);
        let tmp = {
            let mut name = cache_path
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_else(|| "book.parse.bin".into());
            name.push(".tmp");
            cache_path.with_file_name(name)
        };
        if let Some(parent) = tmp.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        {
            let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            file.write_all(&out).map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, &cache_path).map_err(|e| e.to_string())?;
        Ok(cache_path)
    }

    pub fn open_or_cached(path: &Path) -> Result<(Self, bool), String> {
        if let Some(book) = Self::load_parse_cache(path) {
            return Ok((book, true));
        }
        let book = Self::open(path)?;
        Ok((book, false))
    }
}
