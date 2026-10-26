//! Paper templates as simple drawing primitives, shared by all renderers.

use crate::geom::Rect;
use crate::model::{Template, mm};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prim {
    /// `margin` lines use the red margin colour.
    Line {
        a: [f32; 2],
        b: [f32; 2],
        w: f32,
        margin: bool,
    },
    Dot {
        c: [f32; 2],
        r: f32,
    },
    Rect {
        r: Rect,
        w: f32,
    },
}

const LW: f32 = 0.6;

fn hline(out: &mut Vec<Prim>, y: f32, x1: f32, x2: f32, w: f32) {
    out.push(Prim::Line {
        a: [x1, y],
        b: [x2, y],
        w,
        margin: false,
    });
}
fn vline(out: &mut Vec<Prim>, x: f32, y1: f32, y2: f32, w: f32) {
    out.push(Prim::Line {
        a: [x, y1],
        b: [x, y2],
        w,
        margin: false,
    });
}

/// Primitives for a page of size w × h (points). `weight` scales line thickness.
pub fn template_prims(t: Template, w: f32, h: f32, weight: f32) -> Vec<Prim> {
    let lw = LW * weight;
    let mut out = Vec::new();
    match t {
        Template::Blank => {}
        Template::Lined | Template::Narrow => {
            let gap = if t == Template::Lined {
                mm(8.7)
            } else {
                mm(7.1)
            };
            let mut y = 72.0;
            while y < h - 24.0 {
                hline(&mut out, y, 0.0, w, lw);
                y += gap;
            }
            out.push(Prim::Line {
                a: [57.0, 0.0],
                b: [57.0, h],
                w: lw,
                margin: true,
            });
        }
        Template::Grid => {
            let g = mm(5.0);
            let mut y = g;
            while y < h {
                hline(&mut out, y, 0.0, w, lw * 0.8);
                y += g;
            }
            let mut x = g;
            while x < w {
                vline(&mut out, x, 0.0, h, lw * 0.8);
                x += g;
            }
        }
        Template::Dotted => {
            let g = mm(5.0);
            let mut y = g;
            while y < h {
                let mut x = g;
                while x < w {
                    out.push(Prim::Dot {
                        c: [x, y],
                        r: 0.95 * weight,
                    });
                    x += g;
                }
                y += g;
            }
        }
        Template::Cornell => {
            let mut y = 100.0;
            while y < h - 170.0 {
                hline(&mut out, y, 150.0, w, lw);
                y += mm(8.0);
            }
            hline(&mut out, 72.0, 0.0, w, lw * 2.0);
            vline(&mut out, 150.0, 72.0, h - 165.0, lw * 2.0);
            hline(&mut out, h - 165.0, 0.0, w, lw * 2.0);
        }
        Template::Todo => {
            let mut y = 84.0;
            while y < h - 30.0 {
                hline(&mut out, y, 44.0, w - 38.0, lw);
                out.push(Prim::Rect {
                    r: Rect::new(48.0, y - 20.0, 13.0, 13.0),
                    w: lw * 1.4,
                });
                y += 30.0;
            }
        }
        Template::Music => {
            let mut top = 68.0;
            while top + 30.0 < h - 30.0 {
                for i in 0..5 {
                    hline(&mut out, top + i as f32 * 6.5, 38.0, w - 38.0, lw);
                }
                top += 66.0;
            }
        }
        Template::Storyboard => {
            let (mx, gx, top) = (38.0, 22.0, 54.0);
            let cw = (w - mx * 2.0 - gx) / 2.0;
            let (ch, gy) = (158.0, 90.0);
            for r in 0..3 {
                for c in 0..2 {
                    let x = mx + c as f32 * (cw + gx);
                    let y = top + r as f32 * (ch + gy);
                    out.push(Prim::Rect {
                        r: Rect::new(x, y, cw, ch),
                        w: lw * 1.4,
                    });
                    hline(&mut out, y + ch + 26.0, x, x + cw, lw);
                    hline(&mut out, y + ch + 52.0, x, x + cw, lw);
                }
            }
        }
    }
    out
}
