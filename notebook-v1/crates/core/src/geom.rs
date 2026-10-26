//! Bounding boxes, hit testing, lasso tests and transforms.

use crate::model::{Content, Item, Shape, ShapeKind};
use crate::text;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn from_points<'a>(pts: impl IntoIterator<Item = &'a [f32; 2]>) -> Self {
        Self::from_xy(pts.into_iter().copied())
    }

    pub fn from_xy(pts: impl IntoIterator<Item = [f32; 2]>) -> Self {
        let (mut x1, mut y1, mut x2, mut y2) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in pts {
            x1 = x1.min(p[0]);
            y1 = y1.min(p[1]);
            x2 = x2.max(p[0]);
            y2 = y2.max(p[1]);
        }
        if x1 > x2 {
            return Rect::default();
        }
        Rect {
            x: x1,
            y: y1,
            w: x2 - x1,
            h: y2 - y1,
        }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> [f32; 2] {
        [self.x + self.w / 2.0, self.y + self.h / 2.0]
    }

    pub fn expand(&self, d: f32) -> Rect {
        Rect {
            x: self.x - d,
            y: self.y - d,
            w: self.w + 2.0 * d,
            h: self.h + 2.0 * d,
        }
    }

    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect {
            x,
            y,
            w: self.right().max(o.right()) - x,
            h: self.bottom().max(o.bottom()) - y,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.x <= o.right() && o.x <= self.right() && self.y <= o.bottom() && o.y <= self.bottom()
    }
}

pub fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

pub fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - (a[0] + t * dx)).hypot(p[1] - (a[1] + t * dy))
}

/// Outline of a shape and whether it is closed.
pub fn shape_outline(s: &Shape) -> (Vec<[f32; 2]>, bool) {
    if s.points.len() < 2 {
        return (s.points.clone(), false);
    }
    let (a, b) = (s.points[0], s.points[1]);
    match s.kind {
        ShapeKind::Line | ShapeKind::Arrow => (vec![a, b], false),
        ShapeKind::Rect => {
            let (x1, x2) = (a[0].min(b[0]), a[0].max(b[0]));
            let (y1, y2) = (a[1].min(b[1]), a[1].max(b[1]));
            (vec![[x1, y1], [x2, y1], [x2, y2], [x1, y2]], true)
        }
        ShapeKind::Ellipse => {
            let (cx, cy) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
            let (rx, ry) = ((b[0] - a[0]).abs() / 2.0, (b[1] - a[1]).abs() / 2.0);
            let n = 72;
            let pts = (0..n)
                .map(|i| {
                    let t = i as f32 / n as f32 * std::f32::consts::TAU;
                    [cx + rx * t.cos(), cy + ry * t.sin()]
                })
                .collect();
            (pts, true)
        }
        ShapeKind::Polygon => (s.points.clone(), true),
    }
}

pub fn item_bbox(it: &Item) -> Rect {
    match &it.content {
        Content::Stroke(s) => {
            Rect::from_xy(s.points.iter().map(|p| [p.x, p.y])).expand(s.width * 1.2)
        }
        Content::Shape(s) => {
            let (pts, _) = shape_outline(s);
            let mut r = Rect::from_points(pts.iter()).expand(s.width / 2.0);
            if s.kind == ShapeKind::Arrow {
                r = r.expand(s.width * 4.0 + 6.0);
            }
            r
        }
        Content::Text(t) => Rect::new(t.x, t.y, t.w, text::text_height(&t.text, t.size, t.w)),
    }
}

pub fn union_bbox<'a>(items: impl IntoIterator<Item = &'a Item>) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    for it in items {
        let b = item_bbox(it);
        acc = Some(match acc {
            Some(a) => a.union(&b),
            None => b,
        });
    }
    acc
}

fn poly_hit(pts: &[[f32; 2]], closed: bool, p: [f32; 2], r: f32) -> bool {
    match pts.len() {
        0 => false,
        1 => dist(pts[0], p) <= r,
        n => {
            let segs = if closed { n } else { n - 1 };
            (0..segs).any(|i| seg_dist(p, pts[i], pts[(i + 1) % n]) <= r)
        }
    }
}

/// Does a circle of radius `r` around (x, y) touch the item?
pub fn hit_item(it: &Item, x: f32, y: f32, r: f32) -> bool {
    let p = [x, y];
    match &it.content {
        Content::Stroke(s) => {
            let pts: Vec<[f32; 2]> = s.points.iter().map(|q| [q.x, q.y]).collect();
            poly_hit(&pts, false, p, r + s.width / 2.0)
        }
        Content::Shape(s) => {
            let (pts, closed) = shape_outline(s);
            poly_hit(&pts, closed, p, r + s.width / 2.0)
        }
        Content::Text(_) => item_bbox(it).expand(r).contains(x, y),
    }
}

pub fn point_in_poly(x: f32, y: f32, poly: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (poly[i][0], poly[i][1]);
        let (xj, yj) = (poly[j][0], poly[j][1]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// An item counts as lassoed when at least half of its points (or its centre) are inside.
pub fn item_in_lasso(it: &Item, poly: &[[f32; 2]]) -> bool {
    let pts: Vec<[f32; 2]> = match &it.content {
        Content::Stroke(s) => s.points.iter().map(|q| [q.x, q.y]).collect(),
        Content::Shape(s) => shape_outline(s).0,
        Content::Text(_) => {
            let c = item_bbox(it).center();
            return point_in_poly(c[0], c[1], poly);
        }
    };
    if pts.is_empty() {
        return false;
    }
    let step = (pts.len() / 48).max(1);
    let (mut inside, mut total) = (0, 0);
    for p in pts.iter().step_by(step) {
        total += 1;
        if point_in_poly(p[0], p[1], poly) {
            inside += 1;
        }
    }
    inside * 2 >= total
}

/// Maps every point of an item through `f`; `s` scales line widths and text size.
pub fn transform_item(it: &Item, f: impl Fn(f32, f32) -> [f32; 2], s: f32) -> Item {
    let mut out = it.clone();
    match &mut out.content {
        Content::Stroke(st) => {
            st.width *= s;
            for p in &mut st.points {
                let q = f(p.x, p.y);
                p.x = q[0];
                p.y = q[1];
            }
        }
        Content::Shape(sh) => {
            sh.width *= s;
            for p in &mut sh.points {
                *p = f(p[0], p[1]);
            }
        }
        Content::Text(t) => {
            let q = f(t.x, t.y);
            t.x = q[0];
            t.y = q[1];
            t.w *= s;
            t.size *= s;
        }
    }
    out
}

pub fn translate_item(it: &Item, dx: f32, dy: f32) -> Item {
    transform_item(it, |x, y| [x + dx, y + dy], 1.0)
}

/// Scales around an anchor point.
pub fn scale_item(it: &Item, ax: f32, ay: f32, s: f32) -> Item {
    transform_item(it, |x, y| [ax + (x - ax) * s, ay + (y - ay) * s], s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn stroke(pts: &[(f32, f32)]) -> Item {
        Item::new(Content::Stroke(Stroke {
            pen: PenKind::Fineliner,
            color: Rgba::BLACK,
            width: 2.0,
            points: pts
                .iter()
                .map(|&(x, y)| InkPoint { x, y, p: 0.5 })
                .collect(),
        }))
    }

    #[test]
    fn hit_and_lasso() {
        let it = stroke(&[(10.0, 10.0), (50.0, 10.0)]);
        assert!(hit_item(&it, 30.0, 12.0, 2.0));
        assert!(!hit_item(&it, 30.0, 30.0, 2.0));
        let lasso = [[0.0, 0.0], [60.0, 0.0], [60.0, 20.0], [0.0, 20.0]];
        assert!(item_in_lasso(&it, &lasso));
        let b = item_bbox(&it);
        assert!(b.x < 10.0 && b.right() > 50.0);
    }

    #[test]
    fn scale_moves_points() {
        let it = stroke(&[(10.0, 10.0), (20.0, 10.0)]);
        let s = scale_item(&it, 10.0, 10.0, 2.0);
        if let Content::Stroke(st) = &s.content {
            assert_eq!(st.points[1].x, 30.0);
            assert_eq!(st.width, 4.0);
        } else {
            panic!()
        }
    }
}
