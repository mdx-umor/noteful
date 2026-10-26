//! Unlimited-feeling undo. Snapshots are cheap because items are shared (`Arc`):
//! a snapshot copies page structs and item pointers, never the ink itself.

use crate::model::Page;

const LIMIT: usize = 500;

#[derive(Default)]
pub struct History {
    undo: Vec<Vec<Page>>,
    redo: Vec<Vec<Page>>,
}

impl History {
    /// Call with the pages as they were *before* a change.
    pub fn push(&mut self, before: Vec<Page>) {
        self.undo.push(before);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, current: Vec<Page>) -> Option<Vec<Page>> {
        let prev = self.undo.pop()?;
        self.redo.push(current);
        Some(prev)
    }

    pub fn redo(&mut self, current: Vec<Page>) -> Option<Vec<Page>> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn undo_redo_round_trip() {
        let mut h = History::default();
        let a = vec![Page::new(Template::Blank, Paper::White)];
        let mut b = a.clone();
        b.push(Page::new(Template::Grid, Paper::White));
        h.push(a.clone());
        let back = h.undo(b.clone()).unwrap();
        assert_eq!(back.len(), 1);
        let fwd = h.redo(back).unwrap();
        assert_eq!(fwd.len(), 2);
        assert!(h.can_undo() && !h.can_redo());
    }
}
