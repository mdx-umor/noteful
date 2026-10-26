//! Files on disk. Every write goes to a temp file first and is then renamed,
//! so a crash mid-save never leaves a half-written notebook.

use crate::library::Library;
use crate::model::{Document, Id, now_ms};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
    exports: PathBuf,
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

impl Store {
    /// Data lives in the app's private storage; exports and backups go where the user
    /// can reach them (on iPad: the app's folder in the Files app).
    pub fn open_default() -> io::Result<Store> {
        if let Some(dir) = std::env::var_os("NOTEBOOK_DATA_DIR") {
            let root = PathBuf::from(dir);
            return Store::new(root.join("data"), root.join("exports"));
        }
        let h = home();
        if cfg!(target_os = "ios") {
            Store::new(
                h.join("Library/Application Support/Notebook"),
                h.join("Documents"),
            )
        } else if cfg!(target_os = "macos") {
            Store::new(
                h.join("Library/Application Support/Notebook"),
                h.join("Documents/Notebook"),
            )
        } else {
            let data = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| h.join(".local/share"));
            Store::new(data.join("notebook-v1"), h.join("Documents/Notebook"))
        }
    }

    pub fn new(root: PathBuf, exports: PathBuf) -> io::Result<Store> {
        fs::create_dir_all(root.join("notebooks"))?;
        Ok(Store { root, exports })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn library_path(&self) -> PathBuf {
        self.root.join("library.json")
    }

    fn doc_path(&self, id: Id) -> PathBuf {
        self.root.join("notebooks").join(format!("{id}.json"))
    }

    pub fn load_library(&self) -> Library {
        fs::read(self.library_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_library(&self, lib: &Library) -> io::Result<()> {
        write_atomic(&self.library_path(), &serde_json::to_vec(lib)?)
    }

    pub fn load_doc(&self, id: Id) -> io::Result<Document> {
        let bytes = fs::read(self.doc_path(id))?;
        serde_json::from_slice(&bytes).map_err(io::Error::other)
    }

    pub fn save_doc(&self, doc: &Document) -> io::Result<()> {
        write_atomic(&self.doc_path(doc.id), &serde_json::to_vec(doc)?)
    }

    pub fn delete_doc(&self, id: Id) -> io::Result<()> {
        match fs::remove_file(self.doc_path(id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    fn thumb_path(&self, id: Id) -> PathBuf {
        self.root.join("thumbs").join(format!("{id}.png"))
    }

    /// Stores a small PNG preview of a notebook's first page.
    pub fn save_thumb(&self, id: Id, png: &[u8]) -> io::Result<()> {
        write_atomic(&self.thumb_path(id), png)
    }

    /// Decoded preview as (width, height, RGBA bytes).
    pub fn load_thumb(&self, id: Id) -> Option<(u32, u32, Vec<u8>)> {
        let bytes = fs::read(self.thumb_path(id)).ok()?;
        let mut reader = png::Decoder::new(io::Cursor::new(bytes)).read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            return None;
        }
        buf.truncate(info.buffer_size());
        Some((info.width, info.height, buf))
    }

    pub fn delete_thumb(&self, id: Id) {
        let _ = fs::remove_file(self.thumb_path(id));
    }

    /// Writes an exported file and returns its path.
    pub fn export(&self, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        let safe: String = name
            .chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
            .collect();
        let path = self.exports.join("Exports").join(safe);
        write_atomic(&path, bytes)?;
        Ok(path)
    }

    /// Copies the whole library into `Backups/<date>` and keeps the newest `keep` copies.
    pub fn backup(&self, keep: usize) -> io::Result<PathBuf> {
        let dir = self.exports.join("Backups");
        let dest = dir.join(format!("backup-{}", now_ms()));
        copy_dir(&self.root, &dest)?;
        let mut existing: Vec<PathBuf> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("backup-"))
            })
            .collect();
        existing.sort();
        while existing.len() > keep {
            let old = existing.remove(0);
            let _ = fs::remove_dir_all(old);
        }
        Ok(dest)
    }

    /// Time of the newest backup, if any.
    pub fn last_backup(&self) -> Option<u64> {
        fs::read_dir(self.exports.join("Backups"))
            .ok()?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.file_name()
                    .to_string_lossy()
                    .strip_prefix("backup-")
                    .and_then(|s| s.parse().ok())
            })
            .max()
    }
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else if e.path().extension().is_none_or(|x| x != "tmp") {
            fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::NotebookMeta;
    use crate::model::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("nb-test-{}", new_id()));
        let store = Store::new(dir.join("data"), dir.join("exports")).unwrap();
        let doc = Document::new(new_id(), Template::Grid, Paper::Cream);
        store.save_doc(&doc).unwrap();
        assert_eq!(store.load_doc(doc.id).unwrap(), doc);
        let mut lib = Library::default();
        lib.notebooks.push(NotebookMeta::new(
            doc.id,
            "T",
            Rgba::BLACK,
            Template::Grid,
            Paper::Cream,
        ));
        store.save_library(&lib).unwrap();
        assert_eq!(store.load_library(), lib);
        let page = &doc.pages[0];
        store
            .save_thumb(doc.id, &crate::render::render_png(page, 120))
            .unwrap();
        let (w, h, px) = store.load_thumb(doc.id).unwrap();
        assert_eq!((w as usize * h as usize * 4), px.len());
        store.backup(2).unwrap();
        assert!(store.last_backup().is_some());
        store.delete_doc(doc.id).unwrap();
        assert!(store.load_doc(doc.id).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
