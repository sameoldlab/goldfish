/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use serde::{Deserialize, Serialize};

/// Maximum columns any SearchItem variant can use.
/// Keep this in sync with the Nucleo column count in main.rs.
pub const MAX_COLUMNS: usize = 2;

/// The trait nucleo uses to get match keys and display strings.
/// Multiple keys per item allow matching on filename, content,
/// and metadata without separate search passes.
pub trait SearchTarget: Send + Sync + 'static {
    /// How many of the columns from match_keys() are valid for this item.
    fn column_count(&self) -> usize;

    /// All strings nucleo should match against, padded to MAX_COLUMNS.
    /// Returns a stack array — no heap allocation.
    fn match_keys(&self) -> [&str; MAX_COLUMNS];

    /// Human-readable string for output.
    fn display(&self) -> &str;

    /// Stable string for the cache item_type column.
    fn type_name(&self) -> &'static str;
}

// ── Phase 1: file paths only ─────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct FileItem {
    /// Full path as returned by the walker.
    pub path: String,
    /// Filename stem — matched separately so "main" ranks above
    /// a path that merely contains the substring "main".
    pub stem: String,
}

impl FileItem {
    pub fn from_path(path: &std::path::Path) -> Self {
        let path_str = path.to_string_lossy().into_owned();
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            path: path_str,
            stem,
        }
    }
}

// ── Unified enum ─────────────────────────────────────────────────────────────

/// Internal enum so the pipeline is a single `Nucleo<SearchItem>`.
/// New search types add a variant here and an Extractor impl.
#[derive(Clone, Serialize, Deserialize)]
pub enum SearchItem {
    File(FileItem),
    // Text(TextItem), Pdf(PdfItem), Image(ImageItem) — future phases
}

impl SearchTarget for SearchItem {
    fn column_count(&self) -> usize {
        match self {
            SearchItem::File(_) => 2,
        }
    }

    fn match_keys(&self) -> [&str; MAX_COLUMNS] {
        match self {
            SearchItem::File(f) => [f.path.as_str(), f.stem.as_str()],
        }
    }

    fn display(&self) -> &str {
        match self {
            SearchItem::File(f) => f.path.as_str(),
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            SearchItem::File(_) => "file",
        }
    }
}
