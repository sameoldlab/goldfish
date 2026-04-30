/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::{path::PathBuf, thread};

use crossbeam_channel::Sender;
use ignore::{WalkBuilder, WalkState};

pub struct WalkOptions {
    pub standard_filters: bool,
    pub hidden: bool,
    pub follow_symlinks: bool,
}

pub struct WalkSource {
    pub root: PathBuf,
    pub options: WalkOptions,
}

impl WalkSource {
    /// Emit paths in parallel into `tx`. Returns when the walk is complete.
    /// No extraction, no stat beyond what ignore does internally.
    pub fn run(&self, tx: Sender<PathBuf>) {
        WalkBuilder::new(&self.root)
            .require_git(false)
            .follow_links(self.options.follow_symlinks)
            .standard_filters(self.options.standard_filters)
            .hidden(!self.options.hidden)
            .threads(thread::available_parallelism().map_or(4, |n| n.get()))
            .build_parallel()
            .run(|| {
                let tx = tx.clone();
                Box::new(move |entry| {
                    let path = match entry {
                        Ok(e) => e.into_path(),
                        Err(_) => return WalkState::Continue,
                    };
                    // skip directories — we only index entries
                    if path.is_dir() {
                        return WalkState::Continue;
                    }
                    // send; if receiver is gone we're shutting down
                    if tx.send(path).is_err() {
                        return WalkState::Quit;
                    }
                    WalkState::Continue
                })
            });
        // tx drops here, closing the channel and signalling extractors to finish
    }
}
