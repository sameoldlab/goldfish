/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    thread,
    time::Instant,
};

use crossbeam_channel::{bounded, unbounded};
use nucleo::Injector;

use crate::{
    cache::{mtime_secs, lookup, CacheReaderFactory, CacheWriter},
    error::PipelineError,
    item::{FileItem, SearchItem, SearchTarget},
    source::WalkSource,
};

/// Write request sent from extractor threads to the single write thread.
struct WriteRequest {
    path: PathBuf,
    mtime: u64,
    size: u64,
    item: SearchItem,
}

pub fn run(
    source: WalkSource,
    reader_factory: Arc<CacheReaderFactory>,
    mut writer: CacheWriter,
    injector: Arc<Injector<SearchItem>>,
) -> Result<(), PipelineError> {
    let (path_tx, path_rx) = bounded::<PathBuf>(8192);
    let (write_tx, write_rx) = unbounded::<WriteRequest>();

    let thread_count = thread::available_parallelism().map_or(4, |n| n.get());

    // ── walker thread ────────────────────────────────────────────────────────
    let walker_handle = thread::spawn(move || {
        source.run(path_tx);
    });

    // ── extractor threads ────────────────────────────────────────────────────
    let extractor_handles: Vec<_> = (0..thread_count)
        .map(|_| {
            let path_rx = path_rx.clone();
            let write_tx = write_tx.clone();
            let reader_factory = reader_factory.clone();
            let injector = injector.clone();

            thread::spawn(move || -> Result<(), PipelineError> {
                let conn = reader_factory.open_reader()?;

                for path in path_rx {
                    let meta = match fs::metadata(&path) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    let mtime = mtime_secs(&meta);
                    let size = meta.len();

                    let item = match lookup(&conn, &path, mtime, size) {
                        Some(cached) => cached,
                        None => {
                            let item = SearchItem::File(FileItem::from_path(&path));
                            let _ = write_tx.send(WriteRequest {
                                path: path.clone(),
                                mtime,
                                size,
                                item: item.clone(),
                            });
                            item
                        }
                    };

                    injector.push(item, move |it, cols| {
                        let keys = it.match_keys();
                        for (col, key) in cols.iter_mut().zip(keys.iter()) {
                            *col = (*key).to_owned().into();
                        }
                    });
                }
                Ok(())
            })
        })
        .collect();

    drop(write_tx);

    // ── write thread ─────────────────────────────────────────────────────────
    let write_handle = thread::spawn(move || -> Result<(), PipelineError> {
        let mut pending: usize = 0;
        let mut last_commit = Instant::now();

        writer.begin()?;

        for req in write_rx {
            writer.store(&req.path, req.mtime, req.size, &req.item)?;
            pending += 1;

            if pending >= 500 || last_commit.elapsed().as_millis() > 100 {
                writer.commit_begin()?;
                pending = 0;
                last_commit = Instant::now();
            }
        }

        if pending > 0 {
            writer.commit()?;
        }

        Ok(())
    });

    // ── join ─────────────────────────────────────────────────────────────────
    walker_handle.join().expect("walker panicked");

    for handle in extractor_handles {
        handle.join().expect("extractor panicked")?;
    }

    write_handle.join().expect("write thread panicked")?;

    Ok(())
}
