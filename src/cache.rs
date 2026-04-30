/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::CacheError;
use crate::item::{SearchItem, SearchTarget};

/// Serializable form stored in the cache blob.
/// Separate from SearchItem so the cache schema is stable
/// even if SearchItem gains fields that don't need persisting.
#[derive(Serialize, Deserialize)]
pub struct CachedItem {
    pub item: SearchItem,
}

/// Owned by the write thread. Not Send, not shared.
pub struct CacheWriter {
    conn: Connection,
}

/// Cheap to clone, Send. Just a path.
#[derive(Clone)]
pub struct CacheReaderFactory {
    db_path: Arc<PathBuf>,
}

pub fn open(path: &Path) -> Result<(CacheWriter, CacheReaderFactory), CacheError> {
    let conn = Connection::open(path)?;

    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA cache_size=-65536;
         PRAGMA temp_store=MEMORY;
         PRAGMA mmap_size=2147483648;",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cache (
            path        BLOB    PRIMARY KEY,
            mtime       INTEGER NOT NULL,
            size        INTEGER NOT NULL,
            item_type   TEXT    NOT NULL,
            data        BLOB    NOT NULL,
            indexed_at  INTEGER NOT NULL
        ) STRICT;",
    )?;

    Ok((
        CacheWriter { conn },
        CacheReaderFactory {
            db_path: Arc::new(path.to_owned()),
        },
    ))
}

impl CacheWriter {
    pub fn begin(&mut self) -> Result<(), CacheError> {
        self.conn.execute_batch("BEGIN;")?;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), CacheError> {
        self.conn.execute_batch("COMMIT;")?;
        Ok(())
    }

    pub fn commit_begin(&mut self) -> Result<(), CacheError> {
        self.conn.execute_batch("COMMIT; BEGIN;")?;
        Ok(())
    }

    pub fn store(
        &mut self,
        path: &Path,
        mtime: u64,
        size: u64,
        item: &SearchItem,
    ) -> Result<(), CacheError> {
        let data = postcard::to_allocvec(&CachedItem { item: item.clone() })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.conn.execute(
            "INSERT OR REPLACE INTO cache (path, mtime, size, item_type, data, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                path.as_os_str().as_encoded_bytes(),
                mtime,
                size,
                item.type_name(),
                data,
                now,
            ],
        )?;

        Ok(())
    }
}

impl CacheReaderFactory {
    pub fn open_reader(&self) -> Result<Connection, CacheError> {
        let conn = Connection::open(&*self.db_path)?;
        conn.execute_batch(
            "PRAGMA query_only=ON;
             PRAGMA mmap_size=2147483648;",
        )?;
        Ok(conn)
    }
}

pub fn lookup(conn: &Connection, path: &Path, mtime: u64, size: u64) -> Option<SearchItem> {
    conn.query_row(
        "SELECT data FROM cache WHERE path=?1 AND mtime=?2 AND size=?3",
        params![path.as_os_str().as_encoded_bytes(), mtime, size],
        |row| row.get::<_, Vec<u8>>(0),
    )
    .ok()
    .and_then(|bytes| postcard::from_bytes::<CachedItem>(&bytes).ok())
    .map(|c| c.item)
}

pub fn mtime_secs(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
