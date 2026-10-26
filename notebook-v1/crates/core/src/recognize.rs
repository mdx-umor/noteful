//! Hold-to-snap shape recognition: a hand-drawn line, triangle, rectangle,
//! quadrilateral or ellipse becomes a clean shape.

use crate::geom::{Rect, dist, seg_dist};
use crate::model::ShapeKind;

/// Ramer–Douglas–Peucker simplification.
pub fn rdp(pts: &[[f32; 2]], eps: f32) -> Vec<[f32; 2]> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let (mut idx, mut max_d) = (0, 0.0);
    for (i, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = seg_dist(*p, a, b);
        if d > max_d {
            max_d = d;
            idx = i;
        }
    }
    if max_d > eps {
        let mut left = rdp(&pts[..=idx], eps);
        let right = rdp(&pts[idx..], eps);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![a, b]
    }
}

pub fn recognize(points: &[[f32; 2]]) -> Option<(ShapeKind, Vec<[f32; 2]>)> {
    if points.len() < 4 {
        return None;
    }
    let bbox = Rect::from_points(points.iter());
    let diag = bbox.w.hypot(bbox.h);
    if diag < 14.0 {
        return None;
    }
    let len: f32 = points.windows(2).map(|w| dist(w[0], w[1])).sum();
    let (first, last) = (points[0], points[points.len() - 1]);
    let ends = dist(first, last);

    if ends / len > 0.9 {
        return Some((ShapeKind::Line, vec![first, last]));
    }
    if ends > diag * 0.3 {
        return None;
    }

    let mut verts = rdp(points, diag * 0.07);
    if verts.len() > 1 && dist(verts[0], verts[verts.len() - 1]) < diag * 0.2 {
        verts.pop();
    }
    let mut merged: Vec<[f32; 2]> = Vec::new();
    for v in verts {
        if merged.last().is_none_or(|m| dist(*m, v) > diag * 0.12) {
            merged.push(v);
        }
    }
    if merged.len() > 2 && dist(merged[0], merged[merged.len() - 1]) < diag * 0.12 {
        merged.pop();
    }

    match merged.len() {
        3 => return Some((ShapeKind::Polygon, merged)),
        4 => {
            let axis_aligned = (0..4).all(|i| {
                let (v, w) = (merged[i], merged[(i + 1) % 4]);
                let ang = (w[1] - v[1]).atan2(w[0] - v[0]).to_degrees().abs() % 90.0;
                !(14.0..=76.0).contains(&ang)
            });
            if axis_aligned {
                return Some((
                    ShapeKind::Rect,
                    vec![[bbox.x, bbox.y], [bbox.right(), bbox.bottom()]],
                ));
            }
            return Some((ShapeKind::Polygon, merged));
        }
        _ => {}
    }

    // A round closed loop becomes an ellipse when its radius is fairly even.
    let (cx, cy) = (bbox.x + bbox.w / 2.0, bbox.y + bbox.h / 2.0);
    let (rx, ry) = ((bbox.w / 2.0).max(1.0), (bbox.h / 2.0).max(1.0));
    let err: f32 = points
        .iter()
        .map(|p| (((p[0] - cx) / rx).hypot((p[1] - cy) / ry) - 1.0).abs())
        .sum::<f32>()
        / points.len() as f32;
    if err < 0.22 {
        return Some((
            ShapeKind::Ellipse,
            vec![[bbox.x, bbox.y], [bbox.right(), bbox.bottom()]],
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line() {
        let pts: Vec<[f32; 2]> = (0..20)
            .map(|i| [i as f32 * 5.0, i as f32 * 2.0 + (i % 2) as f32 * 0.5])
            .collect();
        assert_eq!(recognize(&pts).unwrap().0, ShapeKind::Line);
    }

    #[test]
    fn circle() {
        let pts: Vec<[f32; 2]> = (0..=60)
            .map(|i| {
                let t = i as f32 / 60.0 * std::f32::consts::TAU;
                [100.0 + 50.0 * t.cos(), 100.0 + 40.0 * t.sin()]
            })
            .collect();
        assert_eq!(recognize(&pts).unwrap().0, ShapeKind::Ellipse);
    }

    #[test]
    fn rectangle() {
        let mut pts = Vec::new();
        let corners = [
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 60.0],
            [0.0, 60.0],
            [0.0, 0.0],
        ];
        for w in corners.windows(2) {
            for k in 0..10 {
                let t = k as f32 / 10.0;
                pts.push([
                    w[0][0] + (w[1][0] - w[0][0]) * t,
                    w[0][1] + (w[1][1] - w[0][1]) * t,
                ]);
            }
        }
        pts.push([1.0, 1.0]);
        assert_eq!(recognize(&pts).unwrap().0, ShapeKind::Rect);
    }

    #[test]
    fn triangle() {
        let mut pts = Vec::new();
        let corners = [[0.0, 100.0], [50.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
        for w in corners.windows(2) {
            for k in 0..12 {
                let t = k as f32 / 12.0;
                pts.push([
                    w[0][0] + (w[1][0] - w[0][0]) * t,
                    w[0][1] + (w[1][1] - w[0][1]) * t,
                ]);
            }
        }
        pts.push([1.0, 99.0]);
        assert_eq!(recognize(&pts).unwrap().0, ShapeKind::Polygon);
    }

    #[test]
    fn scribble_is_not_a_shape() {
        let pts: Vec<[f32; 2]> = (0..40)
            .map(|i| [i as f32 * 3.0, ((i * 7) % 13) as f32 * 4.0])
            .collect();
        assert!(recognize(&pts).is_none());
    }
}
