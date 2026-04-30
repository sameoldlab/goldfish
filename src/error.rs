/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialize(#[from] postcard::Error),
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("cache error: {0}")]
    Cache(#[from] CacheError),
}
