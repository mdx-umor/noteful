//! Ink geometry. Raw pencil points become a smooth centre line with a width at
//! every sample; the width model is what gives each pen its feel. The same
//! samples feed the GPU mesh, the thumbnail rasteriser and the PDF writer.

use crate::geom::shape_outline;
use crate::model::{Paper, PenKind, Rgba, Shape, ShapeKind, Stroke};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

/// How strongly pressure is curved: < 1 makes light touches register sooner.
const PRESSURE_GAMMA: f32 = 0.75;
/// Fountain-pen nib angle (radians). Strokes across the nib are thick, along it thin.
const NIB_ANGLE: f32 = 0.70;

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Turns a stroke into evenly spaced samples along a Catmull-Rom curve.
pub fn samples(s: &Stroke) -> Vec<Sample> {
    // 1. Drop points that barely moved (they only add jitter).
    let mut pts: Vec<[f32; 3]> = Vec::with_capacity(s.points.len());
    for p in &s.points {
        if let Some(l) = pts.last()
            && (p.x - l[0]).hypot(p.y - l[1]) < 0.25
        {
            continue;
        }
        pts.push([p.x, p.y, p.p]);
    }
    if let (Some(last), Some(orig)) = (pts.last_mut(), s.points.last()) {
        last[2] = last[2].max(orig.p.min(last[2] + 0.2));
    }
    if pts.is_empty() {
        return Vec::new();
    }
    if pts.len() == 1 {
        let w = width_at(s, pts[0][2], 0.0, 1.0, 0.0);
        return vec![Sample {
            x: pts[0][0],
            y: pts[0][1],
            w,
        }];
    }

    // 2. Catmull-Rom subdivision for a smooth line between input points.
    let n = pts.len();
    let mut curve: Vec<[f32; 3]> = Vec::with_capacity(n * 3);
    for i in 0..n - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(n - 1)];
        let len = (p2[0] - p1[0]).hypot(p2[1] - p1[1]);
        let steps = ((len / 1.5).ceil() as usize).clamp(1, 16);
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let cr = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            curve.push([
                cr(p0[0], p1[0], p2[0], p3[0]),
                cr(p0[1], p1[1], p2[1], p3[1]),
                p1[2] + (p2[2] - p1[2]) * t,
            ]);
        }
    }
    curve.push(pts[n - 1]);

    // 3. Smooth pressure with a centred window so the line has no lag.
    let raw: Vec<f32> = curve.iter().map(|c| c[2]).collect();
    let m = raw.len();
    let half = 3usize;
    let pressure: Vec<f32> = (0..m)
        .map(|i| {
            let (a, b) = (i.saturating_sub(half), (i + half).min(m - 1));
            raw[a..=b].iter().sum::<f32>() / (b - a + 1) as f32
        })
        .collect();

    let (pmin, pmax) = raw
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let has_pressure = pmax - pmin > 0.04;

    // 4. Arc length for end tapering.
    let mut arc = vec![0.0f32; m];
    for i in 1..m {
        arc[i] = arc[i - 1] + (curve[i][0] - curve[i - 1][0]).hypot(curve[i][1] - curve[i - 1][1]);
    }
    let total = arc[m - 1].max(0.001);

    let mut out: Vec<Sample> = (0..m)
        .map(|i| {
            let a = curve[i.saturating_sub(1)];
            let b = curve[(i + 1).min(m - 1)];
            let dir = (b[1] - a[1]).atan2(b[0] - a[0]);
            let taper_len = (total * 0.22).min(s.width * 7.0).max(0.001);
            let from_end = arc[i].min(total - arc[i]);
            let taper = smoothstep(from_end / taper_len);
            let tapering = match s.pen {
                PenKind::Brush => true,
                PenKind::Fountain => !has_pressure,
                _ => false,
            };
            let t = if tapering { taper } else { 1.0 };
            Sample {
                x: curve[i][0],
                y: curve[i][1],
                w: width_at(s, pressure[i], dir, t, i as f32),
            }
        })
        .collect();

    // 5. Light width smoothing so thick/thin transitions stay soft.
    if matches!(s.pen, PenKind::Fountain | PenKind::Brush) && out.len() > 2 {
        let ws: Vec<f32> = out.iter().map(|s| s.w).collect();
        for i in 1..out.len() - 1 {
            out[i].w = (ws[i - 1] + 2.0 * ws[i] + ws[i + 1]) / 4.0;
        }
    }
    out
}

fn width_at(s: &Stroke, pressure: f32, dir: f32, taper: f32, _i: f32) -> f32 {
    let p = pressure.clamp(0.0, 1.0).powf(PRESSURE_GAMMA);
    let w = s.width;
    let v = match s.pen {
        PenKind::Fineliner => w * (0.82 + 0.36 * p),
        PenKind::Fountain => {
            let nib = 0.42 + 0.58 * (dir - NIB_ANGLE).sin().abs();
            w * (0.35 + 1.15 * p) * nib * (0.35 + 0.65 * taper)
        }
        PenKind::Brush => w * (0.15 + 1.75 * p) * (0.12 + 0.88 * taper),
        PenKind::Highlighter => w,
    };
    v.max(0.25)
}

/// Colour actually painted. Highlighter ink is pre-blended with the paper so it can be
/// drawn opaque underneath the writing: no darker overlaps, no alpha needed anywhere.
pub fn ink_color(s: &Stroke, paper: Paper) -> Rgba {
    if s.pen == PenKind::Highlighter {
        let t = if paper.is_dark() { 0.55 } else { 0.42 };
        s.color.mix_over(paper.background(), t)
    } else {
        s.color
    }
}

/// Polylines (each a list of points, closed ones repeat their first point) for a shape,
/// including arrow heads and dashes.
pub fn shape_polylines(sh: &Shape) -> Vec<Vec<[f32; 2]>> {
    let (mut pts, closed) = shape_outline(sh);
    if pts.is_empty() {
        return Vec::new();
    }
    if closed {
        pts.push(pts[0]);
    }
    let mut lines = if sh.dashed {
        dash(&pts, sh.width * 3.5, sh.width * 2.5)
    } else {
        vec![pts]
    };
    if sh.kind == ShapeKind::Arrow && sh.points.len() >= 2 {
        let (a, b) = (sh.points[0], sh.points[1]);
        let ang = (b[1] - a[1]).atan2(b[0] - a[0]);
        let l = (sh.width * 4.0).max(10.0);
        let wing = |da: f32| [b[0] - l * (ang + da).cos(), b[1] - l * (ang + da).sin()];
        lines.push(vec![wing(-0.45), b, wing(0.45)]);
    }
    lines
}

fn dash(pts: &[[f32; 2]], on: f32, off: f32) -> Vec<Vec<[f32; 2]>> {
    let mut out = Vec::new();
    let mut cur: Vec<[f32; 2]> = vec![pts[0]];
    let mut drawing = true;
    let mut left = on;
    for w in pts.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        let mut seg = (b[0] - a[0]).hypot(b[1] - a[1]);
        while seg > 0.0 {
            let step = seg.min(left);
            let t = step / seg;
            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if drawing {
                cur.push(p);
            }
            left -= step;
            seg -= step;
            a = p;
            if left <= 1e-4 {
                if drawing {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur = vec![p];
                }
                drawing = !drawing;
                left = if drawing { on } else { off };
            }
        }
    }
    if drawing && cur.len() > 1 {
        out.push(cur);
    }
    out
}

/// Triangle mesh in page coordinates.
#[derive(Clone, Default, Debug)]
pub struct Mesh {
    pub verts: Vec<[f32; 2]>,
    pub idx: Vec<u32>,
}

impl Mesh {
    pub fn clear(&mut self) {
        self.verts.clear();
        self.idx.clear();
    }

    fn disc(&mut self, c: [f32; 2], r: f32, segs: usize) {
        let base = self.verts.len() as u32;
        self.verts.push(c);
        for k in 0..segs {
            let t = k as f32 / segs as f32 * std::f32::consts::TAU;
            self.verts.push([c[0] + r * t.cos(), c[1] + r * t.sin()]);
        }
        for k in 0..segs as u32 {
            self.idx
                .extend_from_slice(&[base, base + 1 + k, base + 1 + (k + 1) % segs as u32]);
        }
    }
}

fn disc_segments(r: f32) -> usize {
    ((r * 2.5) as usize).clamp(10, 40)
}

/// Appends a variable-width ribbon with round ends and round corners.
pub fn ink_mesh(samples: &[Sample], out: &mut Mesh) {
    match samples.len() {
        0 => {}
        1 => {
            let s = samples[0];
            out.disc([s.x, s.y], s.w / 2.0, disc_segments(s.w / 2.0));
        }
        n => {
            let base = out.verts.len() as u32;
            for i in 0..n {
                let a = samples[i.saturating_sub(1)];
                let b = samples[(i + 1).min(n - 1)];
                let (mut tx, mut ty) = (b.x - a.x, b.y - a.y);
                let l = tx.hypot(ty).max(1e-6);
                tx /= l;
                ty /= l;
                let (nx, ny) = (-ty, tx);
                let s = samples[i];
                let h = s.w / 2.0;
                out.verts.push([s.x + nx * h, s.y + ny * h]);
                out.verts.push([s.x - nx * h, s.y - ny * h]);
                if i > 0 {
                    let k = base + (i as u32 - 1) * 2;
                    out.idx
                        .extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
                }
            }
            // Round caps.
            for s in [samples[0], samples[n - 1]] {
                out.disc([s.x, s.y], s.w / 2.0, disc_segments(s.w / 2.0));
            }
            // Round joins where the line turns sharply.
            for i in 1..n - 1 {
                let (a, b, c) = (samples[i - 1], samples[i], samples[i + 1]);
                let (ux, uy) = (b.x - a.x, b.y - a.y);
                let (vx, vy) = (c.x - b.x, c.y - b.y);
                let cos = (ux * vx + uy * vy) / (ux.hypot(uy) * vx.hypot(vy)).max(1e-6);
                if cos < 0.8 {
                    out.disc([b.x, b.y], b.w / 2.0, disc_segments(b.w / 2.0));
                }
            }
        }
    }
}

pub fn polyline_samples(pts: &[[f32; 2]], w: f32) -> Vec<Sample> {
    // Corners must stay sharp-ish but round: duplicate nothing, the join discs handle it.
    pts.iter()
        .map(|p| Sample {
            x: p[0],
            y: p[1],
            w,
        })
        .collect()
}

pub fn shape_mesh(sh: &Shape, out: &mut Mesh) {
    for line in shape_polylines(sh) {
        ink_mesh(&polyline_samples(&line, sh.width), out);
    }
}

/// Filled interior of a closed shape as a triangle fan (shapes are convex or near-convex).
pub fn shape_fill_mesh(sh: &Shape, out: &mut Mesh) {
    let (pts, closed) = shape_outline(sh);
    if !closed || pts.len() < 3 {
        return;
    }
    let base = out.verts.len() as u32;
    out.verts.extend_from_slice(&pts);
    for k in 1..pts.len() as u32 - 1 {
        out.idx.extend_from_slice(&[base, base + k, base + k + 1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn stroke(pen: PenKind, pressures: &[f32]) -> Stroke {
        Stroke {
            pen,
            color: Rgba::BLACK,
            width: 3.0,
            points: pressures
                .iter()
                .enumerate()
                .map(|(i, &p)| InkPoint {
                    x: i as f32 * 5.0,
                    y: 0.0,
                    p,
                })
                .collect(),
        }
    }

    #[test]
    fn samples_are_dense_and_finite() {
        let s = stroke(PenKind::Fountain, &[0.2, 0.4, 0.9, 0.9, 0.3]);
        let ss = samples(&s);
        assert!(ss.len() > 5);
        assert!(ss.iter().all(|s| s.w.is_finite() && s.w > 0.0));
    }

    #[test]
    fn pressure_changes_width() {
        let light = samples(&stroke(PenKind::Brush, &[0.1; 6]));
        let hard = samples(&stroke(PenKind::Brush, &[1.0; 6]));
        let mid = |v: &Vec<Sample>| v[v.len() / 2].w;
        assert!(mid(&hard) > mid(&light) * 2.0);
    }

    #[test]
    fn mesh_has_triangles() {
        let mut m = Mesh::default();
        ink_mesh(&samples(&stroke(PenKind::Fineliner, &[0.5; 4])), &mut m);
        assert!(m.idx.len() >= 6 && m.idx.len() % 3 == 0);
        assert!(m.idx.iter().all(|&i| (i as usize) < m.verts.len()));
    }

    #[test]
    fn dashes_split_lines() {
        let sh = Shape {
            kind: ShapeKind::Line,
            points: vec![[0.0, 0.0], [100.0, 0.0]],
            color: Rgba::BLACK,
            width: 2.0,
            fill: None,
            dashed: true,
        };
        assert!(shape_polylines(&sh).len() > 5);
    }
}
