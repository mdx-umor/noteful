//! Files on disk. Every write goes to a temp file first and is then renamed,
//! so a crash mid-save never leaves a half-written notebook.

use crate::library::Library;
use crate::model::{Document, Id, now_ms};
#[cfg(not(target_arch = "wasm32"))]
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

/// File access. Native builds use the file system; the web build keeps the same
/// "files" in the browser's local storage, keyed by path.
#[cfg(not(target_arch = "wasm32"))]
mod disk {
    use std::fs;
    use std::io;
    use std::path::Path;

    pub fn read(path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, path)
    }

    pub fn remove(path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    pub fn create_dir_all(path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }
}

#[cfg(target_arch = "wasm32")]
mod disk {
    use std::io;
    use std::path::Path;
    use wasm_bindgen::prelude::*;

    // The page keeps every file in IndexedDB and mirrors it in memory, so reads are
    // instant and there is no 5 MB limit (see web/template.htm, `NBFS`).
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = NBFS, js_name = get)]
        fn nbfs_get(key: &str) -> Option<String>;
        #[wasm_bindgen(js_namespace = NBFS, js_name = set)]
        fn nbfs_set(key: &str, value: &str) -> bool;
        #[wasm_bindgen(js_namespace = NBFS, js_name = del)]
        fn nbfs_del(key: &str);
    }

    fn key(path: &Path) -> String {
        format!("notebook-v1:{}", path.display())
    }

    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn b64_encode(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for c in data.chunks(3) {
            let n = (c[0] as u32) << 16
                | (*c.get(1).unwrap_or(&0) as u32) << 8
                | *c.get(2).unwrap_or(&0) as u32;
            for i in 0..4 {
                if i <= c.len() {
                    out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    fn b64_decode(s: &str) -> Option<Vec<u8>> {
        let mut table = [255u8; 256];
        for (i, &b) in B64.iter().enumerate() {
            table[b as usize] = i as u8;
        }
        let mut out = Vec::with_capacity(s.len() / 4 * 3);
        let mut buf = 0u32;
        let mut bits = 0;
        for ch in s.bytes() {
            if ch == b'=' {
                break;
            }
            let v = table[ch as usize];
            if v == 255 {
                return None;
            }
            buf = buf << 6 | v as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
            }
        }
        Some(out)
    }

    pub fn read(path: &Path) -> io::Result<Vec<u8>> {
        let v = nbfs_get(&key(path))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not found"))?;
        if let Some(t) = v.strip_prefix("t:") {
            Ok(t.as_bytes().to_vec())
        } else if let Some(b) = v.strip_prefix("b:") {
            b64_decode(b).ok_or_else(|| io::Error::other("corrupt data"))
        } else {
            Err(io::Error::other("corrupt data"))
        }
    }

    pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let v = match std::str::from_utf8(bytes) {
            Ok(t) => format!("t:{t}"),
            Err(_) => format!("b:{}", b64_encode(bytes)),
        };
        if nbfs_set(&key(path), &v) {
            Ok(())
        } else {
            Err(io::Error::other("browser storage is full"))
        }
    }

    pub fn remove(path: &Path) -> io::Result<()> {
        nbfs_del(&key(path));
        Ok(())
    }

    pub fn create_dir_all(_path: &Path) -> io::Result<()> {
        Ok(())
    }
}

impl Store {
    /// Data lives in the app's private storage; exports and backups go where the user
    /// can reach them (on iPad: the app's folder in the Files app).
    pub fn open_default() -> io::Result<Store> {
        if cfg!(target_arch = "wasm32") {
            return Store::new(PathBuf::from("data"), PathBuf::from("exports"));
        }
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
        disk::create_dir_all(&root.join("notebooks"))?;
        Ok(Store { root, exports })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads a small file kept next to the library (settings, flags).
    pub fn read_file(&self, name: &str) -> Option<Vec<u8>> {
        disk::read(&self.root.join(name)).ok()
    }

    pub fn write_file(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        disk::write(&self.root.join(name), bytes)
    }

    fn library_path(&self) -> PathBuf {
        self.root.join("library.json")
    }

    fn doc_path(&self, id: Id) -> PathBuf {
        self.root.join("notebooks").join(format!("{id}.json"))
    }

    pub fn load_library(&self) -> Library {
        disk::read(&self.library_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_library(&self, lib: &Library) -> io::Result<()> {
        disk::write(&self.library_path(), &serde_json::to_vec(lib)?)
    }

    pub fn load_doc(&self, id: Id) -> io::Result<Document> {
        let bytes = disk::read(&self.doc_path(id))?;
        serde_json::from_slice(&bytes).map_err(io::Error::other)
    }

    pub fn save_doc(&self, doc: &Document) -> io::Result<()> {
        disk::write(&self.doc_path(doc.id), &serde_json::to_vec(doc)?)
    }

    pub fn delete_doc(&self, id: Id) -> io::Result<()> {
        disk::remove(&self.doc_path(id))
    }

    fn thumb_path(&self, id: Id) -> PathBuf {
        self.root.join("thumbs").join(format!("{id}.png"))
    }

    /// Stores a small PNG preview of a notebook's first page.
    pub fn save_thumb(&self, id: Id, png: &[u8]) -> io::Result<()> {
        disk::write(&self.thumb_path(id), png)
    }

    /// Decoded preview as (width, height, RGBA bytes).
    pub fn load_thumb(&self, id: Id) -> Option<(u32, u32, Vec<u8>)> {
        let bytes = disk::read(&self.thumb_path(id)).ok()?;
        let mut reader = png::Decoder::new(io::Cursor::new(bytes)).read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            return None;
        }
        buf.truncate(info.buffer_size());
        Some((info.width, info.height, buf))
    }

    fn asset_path(&self, id: Id) -> PathBuf {
        self.root.join("assets").join(format!("{id}.jpg"))
    }

    /// Stores a picture (JPEG bytes) and returns its id.
    pub fn save_asset(&self, bytes: &[u8]) -> io::Result<Id> {
        let id = crate::model::new_id();
        disk::write(&self.asset_path(id), bytes)?;
        Ok(id)
    }

    pub fn load_asset(&self, id: Id) -> Option<Vec<u8>> {
        disk::read(&self.asset_path(id)).ok()
    }

    pub fn delete_asset(&self, id: Id) {
        let _ = disk::remove(&self.asset_path(id));
    }

    pub fn delete_thumb(&self, id: Id) {
        let _ = disk::remove(&self.thumb_path(id));
    }

    /// Writes an exported file and returns its path.
    pub fn export(&self, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        let safe: String = name
            .chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
            .collect();
        let path = self.exports.join("Exports").join(safe);
        disk::write(&path, bytes)?;
        Ok(path)
    }

    /// Copies the whole library into `Backups/<date>` and keeps the newest `keep` copies.
    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(target_arch = "wasm32")]
    pub fn backup(&self, _keep: usize) -> io::Result<PathBuf> {
        Err(io::Error::other("backups are not available in the browser"))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn last_backup(&self) -> Option<u64> {
        Some(now_ms())
    }

    /// Time of the newest backup, if any.
    #[cfg(not(target_arch = "wasm32"))]
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

impl crate::assets::AssetSource for Store {
    fn image(&self, id: Id) -> Option<std::sync::Arc<crate::assets::RgbaImage>> {
        let bytes = self.load_asset(id)?;
        let img = crate::assets::decode_jpeg(&bytes)?;
        Some(std::sync::Arc::new(crate::assets::downscale(&img, 1400)))
    }
}

#[cfg(not(target_arch = "wasm32"))]
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
