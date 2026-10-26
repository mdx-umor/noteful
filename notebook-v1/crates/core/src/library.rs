//! The library: notebook cards, folders, nested #tags, trash and search.

use crate::model::{Id, Paper, Rgba, Template, now_ms};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotebookMeta {
    pub id: Id,
    pub title: String,
    pub cover: Rgba,
    #[serde(default)]
    pub folder: Option<Id>,
    /// Tags like `School` or nested `School/Physics`.
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub trashed_at: Option<u64>,
    pub created: u64,
    pub updated: u64,
    #[serde(default)]
    pub opened: u64,
    pub page_count: usize,
    #[serde(default)]
    pub search_text: String,
    pub template: Template,
    pub paper: Paper,
}

impl NotebookMeta {
    pub fn new(id: Id, title: &str, cover: Rgba, template: Template, paper: Paper) -> Self {
        let now = now_ms();
        NotebookMeta {
            id,
            title: title.to_string(),
            cover,
            folder: None,
            tags: Vec::new(),
            pinned: false,
            trashed_at: None,
            created: now,
            updated: now,
            opened: now,
            page_count: 1,
            search_text: String::new(),
            template,
            paper,
        }
    }

    pub fn trashed(&self) -> bool {
        self.trashed_at.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: Id,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Filter {
    All,
    Untagged,
    Unfiled,
    Trashed,
    Pinned,
    Folder(Id),
    Tag(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortBy {
    Modified,
    Opened,
    Created,
    Name,
}

impl SortBy {
    pub const ALL: [SortBy; 4] = [
        SortBy::Modified,
        SortBy::Opened,
        SortBy::Created,
        SortBy::Name,
    ];
    pub fn name(self) -> &'static str {
        match self {
            SortBy::Modified => "Date modified",
            SortBy::Opened => "Last opened",
            SortBy::Created => "Date created",
            SortBy::Name => "Name",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TagNode {
    pub name: String,
    pub path: String,
    pub count: usize,
    pub children: Vec<TagNode>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub notebooks: Vec<NotebookMeta>,
    pub folders: Vec<Folder>,
}

/// Cleans user input into a tag path: `#School / Physics ` → `School/Physics`.
pub fn normalize_tag(s: &str) -> Option<String> {
    let parts: Vec<&str> = s
        .trim()
        .trim_start_matches('#')
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn tag_matches(tag: &str, path: &str) -> bool {
    tag == path
        || (tag.len() > path.len() && tag.starts_with(path) && tag.as_bytes()[path.len()] == b'/')
}

impl Library {
    pub fn get(&self, id: Id) -> Option<&NotebookMeta> {
        self.notebooks.iter().find(|m| m.id == id)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut NotebookMeta> {
        self.notebooks.iter_mut().find(|m| m.id == id)
    }

    pub fn matches(m: &NotebookMeta, f: &Filter) -> bool {
        match f {
            Filter::Trashed => m.trashed(),
            _ if m.trashed() => false,
            Filter::All => true,
            Filter::Untagged => m.tags.is_empty(),
            Filter::Unfiled => m.folder.is_none(),
            Filter::Pinned => m.pinned,
            Filter::Folder(id) => m.folder == Some(*id),
            Filter::Tag(p) => m.tags.iter().any(|t| tag_matches(t, p)),
        }
    }

    pub fn count(&self, f: &Filter) -> usize {
        self.notebooks
            .iter()
            .filter(|m| Self::matches(m, f))
            .count()
    }

    /// Notebooks for a filter; every search word must appear in the title, a tag or typed text.
    pub fn query(&self, f: &Filter, search: &str, sort: SortBy) -> Vec<&NotebookMeta> {
        let words: Vec<String> = search
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();
        let mut out: Vec<&NotebookMeta> = self
            .notebooks
            .iter()
            .filter(|m| {
                // Searching looks through the whole library (not the trash) like a global search.
                let base = if words.is_empty() {
                    Self::matches(m, f)
                } else {
                    !m.trashed() || *f == Filter::Trashed
                };
                base && words.iter().all(|w| {
                    m.title.to_lowercase().contains(w)
                        || m.tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(w.trim_start_matches('#')))
                        || m.search_text.to_lowercase().contains(w)
                })
            })
            .collect();
        match sort {
            SortBy::Modified => out.sort_by_key(|m| std::cmp::Reverse(m.updated)),
            SortBy::Opened => out.sort_by_key(|m| std::cmp::Reverse(m.opened)),
            SortBy::Created => out.sort_by_key(|m| std::cmp::Reverse(m.created)),
            SortBy::Name => out.sort_by_key(|a| a.title.to_lowercase()),
        }
        // Pinned notebooks float to the top everywhere except the trash.
        if *f != Filter::Trashed {
            out.sort_by_key(|m| !m.pinned);
        }
        out
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .notebooks
            .iter()
            .filter(|m| !m.trashed())
            .flat_map(|m| m.tags.iter().cloned())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn tag_tree(&self) -> Vec<TagNode> {
        let mut paths: Vec<String> = Vec::new();
        for t in self.all_tags() {
            let parts: Vec<&str> = t.split('/').collect();
            for i in 1..=parts.len() {
                paths.push(parts[..i].join("/"));
            }
        }
        paths.sort();
        paths.dedup();
        fn build(lib: &Library, paths: &[String], parent: Option<&str>) -> Vec<TagNode> {
            paths
                .iter()
                .filter(|p| match parent {
                    None => !p.contains('/'),
                    Some(par) => {
                        p.starts_with(par)
                            && p.len() > par.len() + 1
                            && p.as_bytes()[par.len()] == b'/'
                            && !p[par.len() + 1..].contains('/')
                    }
                })
                .map(|p| TagNode {
                    name: p.rsplit('/').next().unwrap_or(p).to_string(),
                    path: p.clone(),
                    count: lib.count(&Filter::Tag(p.clone())),
                    children: build(lib, paths, Some(p)),
                })
                .collect()
        }
        build(self, &paths, None)
    }

    /// Renames a tag and everything nested under it.
    pub fn rename_tag(&mut self, old: &str, new: &str) {
        let Some(new) = normalize_tag(new) else {
            return;
        };
        for m in &mut self.notebooks {
            for t in &mut m.tags {
                if tag_matches(t, old) {
                    *t = format!("{new}{}", &t[old.len()..]);
                }
            }
            m.tags.sort();
            m.tags.dedup();
        }
    }

    pub fn delete_tag(&mut self, path: &str) {
        for m in &mut self.notebooks {
            m.tags.retain(|t| !tag_matches(t, path));
        }
    }

    pub fn delete_folder(&mut self, id: Id) {
        self.folders.retain(|f| f.id != id);
        for m in &mut self.notebooks {
            if m.folder == Some(id) {
                m.folder = None;
            }
        }
    }

    /// Ids of trashed notebooks older than `days`.
    pub fn expired_trash(&self, days: u64) -> Vec<Id> {
        let cutoff = now_ms().saturating_sub(days * 86_400_000);
        self.notebooks
            .iter()
            .filter(|m| m.trashed_at.is_some_and(|t| t < cutoff))
            .map(|m| m.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> Library {
        let mut l = Library::default();
        let mut a = NotebookMeta::new(
            1,
            "Physics notes",
            Rgba::BLACK,
            Template::Lined,
            Paper::White,
        );
        a.tags = vec!["School/Physics".into()];
        let mut b = NotebookMeta::new(2, "Sketches", Rgba::BLACK, Template::Blank, Paper::White);
        b.tags = vec!["Sample/Drawing".into()];
        b.pinned = true;
        let mut c = NotebookMeta::new(3, "Old", Rgba::BLACK, Template::Blank, Paper::White);
        c.trashed_at = Some(1);
        let d = NotebookMeta::new(4, "Groceries", Rgba::BLACK, Template::Todo, Paper::White);
        l.notebooks = vec![a, b, c, d];
        l
    }

    #[test]
    fn filters_and_counts() {
        let l = lib();
        assert_eq!(l.count(&Filter::All), 3);
        assert_eq!(l.count(&Filter::Trashed), 1);
        assert_eq!(l.count(&Filter::Untagged), 1);
        assert_eq!(l.count(&Filter::Pinned), 1);
        assert_eq!(l.count(&Filter::Tag("School".into())), 1);
        assert_eq!(l.count(&Filter::Tag("Sch".into())), 0);
    }

    #[test]
    fn tag_tree_nests() {
        let tree = lib().tag_tree();
        assert_eq!(tree.len(), 2);
        let school = tree.iter().find(|n| n.name == "School").unwrap();
        assert_eq!(school.children[0].name, "Physics");
        assert_eq!(school.count, 1);
    }

    #[test]
    fn search_and_pins() {
        let l = lib();
        let r = l.query(&Filter::All, "phys", SortBy::Name);
        assert_eq!(r.len(), 1);
        let all = l.query(&Filter::All, "", SortBy::Name);
        assert!(all[0].pinned);
    }

    #[test]
    fn rename_nested_tag() {
        let mut l = lib();
        l.rename_tag("School", "Uni");
        assert_eq!(l.notebooks[0].tags, vec!["Uni/Physics".to_string()]);
        assert_eq!(normalize_tag(" #a / b /"), Some("a/b".into()));
    }
}
