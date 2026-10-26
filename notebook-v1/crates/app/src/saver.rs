//! Writes to disk on a background thread so saving never interrupts writing.
//! Jobs run in order; repeated saves of the same notebook collapse to the newest.
//! Browsers have no threads, so the web build saves straight away instead.

use notebook_core::library::Library;
use notebook_core::storage::Store;
use notebook_core::{Document, Id, Page, render};
use std::sync::mpsc::{Receiver, Sender, channel};

pub enum Job {
    Doc(Document),
    Library(Library),
    Thumb(Id, Page),
    /// `true` reports the result to the user (manual backups).
    Backup(bool),
}

pub struct Saver {
    #[cfg(not(target_arch = "wasm32"))]
    tx: Sender<Job>,
    #[cfg(target_arch = "wasm32")]
    store: Store,
    #[cfg(target_arch = "wasm32")]
    done_tx: Sender<String>,
    done_rx: Receiver<String>,
}

/// Runs a batch of jobs, keeping only the newest save of each notebook and of the library.
fn process(store: &Store, batch: Vec<Job>, done_tx: &Sender<String>) {
    let mut docs: Vec<Document> = Vec::new();
    let mut lib: Option<Library> = None;
    let mut rest = Vec::new();
    for j in batch {
        match j {
            Job::Doc(d) => {
                docs.retain(|x| x.id != d.id);
                docs.push(d);
            }
            Job::Library(l) => lib = Some(l),
            other => rest.push(other),
        }
    }
    for d in docs {
        if let Err(e) = store.save_doc(&d) {
            let _ = done_tx.send(format!("Could not save notebook: {e}"));
        }
    }
    if let Some(l) = lib
        && let Err(e) = store.save_library(&l)
    {
        let _ = done_tx.send(format!("Could not save library: {e}"));
    }
    for j in rest {
        match j {
            Job::Thumb(id, page) => {
                let png = render::render_png(&page, 320);
                let _ = store.save_thumb(id, &png);
                let _ = done_tx.send(format!("thumb:{id}"));
            }
            Job::Backup(notify) => match store.backup(7) {
                Ok(_) => {
                    if notify {
                        let _ = done_tx.send("Backup saved to Files › Notebook › Backups".into());
                    }
                }
                Err(e) => {
                    if notify {
                        let _ = done_tx.send(format!("Backup failed: {e}"));
                    }
                }
            },
            Job::Doc(_) | Job::Library(_) => {}
        }
    }
}

impl Saver {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start(store: Store) -> Saver {
        let (tx, rx) = channel::<Job>();
        let (done_tx, done_rx) = channel::<String>();
        std::thread::Builder::new()
            .name("saver".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    let mut batch = vec![first];
                    while let Ok(j) = rx.try_recv() {
                        batch.push(j);
                    }
                    process(&store, batch, &done_tx);
                }
            })
            .expect("spawn saver thread");
        Saver { tx, done_rx }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start(store: Store) -> Saver {
        let (done_tx, done_rx) = channel::<String>();
        Saver {
            store,
            done_tx,
            done_rx,
        }
    }

    pub fn send(&self, job: Job) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.tx.send(job);
        #[cfg(target_arch = "wasm32")]
        process(&self.store, vec![job], &self.done_tx);
    }

    /// Messages from finished jobs (errors, "thumb:<id>").
    pub fn poll(&self) -> Vec<String> {
        self.done_rx.try_iter().collect()
    }
}
