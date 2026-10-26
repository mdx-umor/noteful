//! Ready-made shapes for the Insert Shape panel and the sample sticker pack.

use crate::model::*;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Square,
    Rounded,
    Pill,
    Circle,
    Diamond,
    Triangle,
    Hexagon,
    Cloud,
    Burst,
    Star,
    Heart,
    Arrow,
}

impl Preset {
    pub const ALL: [Preset; 12] = [
        Preset::Square,
        Preset::Rounded,
        Preset::Pill,
        Preset::Circle,
        Preset::Diamond,
        Preset::Triangle,
        Preset::Hexagon,
        Preset::Cloud,
        Preset::Burst,
        Preset::Star,
        Preset::Heart,
        Preset::Arrow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Preset::Square => "Square",
            Preset::Rounded => "Rounded square",
            Preset::Pill => "Pill",
            Preset::Circle => "Circle",
            Preset::Diamond => "Diamond",
            Preset::Triangle => "Triangle",
            Preset::Hexagon => "Hexagon",
            Preset::Cloud => "Cloud",
            Preset::Burst => "Burst",
            Preset::Star => "Star",
            Preset::Heart => "Heart",
            Preset::Arrow => "Arrow",
        }
    }
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<[f32; 2]> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pts = Vec::new();
    let corners = [
        ([x + w - r, y + r], -PI / 2.0),
        ([x + w - r, y + h - r], 0.0),
        ([x + r, y + h - r], PI / 2.0),
        ([x + r, y + r], PI),
    ];
    for (c, a0) in corners {
        for k in 0..=8 {
            let a = a0 + k as f32 / 8.0 * PI / 2.0;
            pts.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        }
    }
    pts
}

fn regular(cx: f32, cy: f32, r: f32, n: usize, rot: f32) -> Vec<[f32; 2]> {
    (0..n)
        .map(|i| {
            let a = rot + i as f32 / n as f32 * TAU;
            [cx + r * a.cos(), cy + r * a.sin()]
        })
        .collect()
}

fn star(cx: f32, cy: f32, r_out: f32, r_in: f32, n: usize) -> Vec<[f32; 2]> {
    (0..n * 2)
        .map(|i| {
            let a = -PI / 2.0 + i as f32 / (n * 2) as f32 * TAU;
            let r = if i % 2 == 0 { r_out } else { r_in };
            [cx + r * a.cos(), cy + r * a.sin()]
        })
        .collect()
}

/// Shape geometry centred on (cx, cy), about `size` points across.
pub fn preset_shape(p: Preset, cx: f32, cy: f32, size: f32) -> (ShapeKind, Vec<[f32; 2]>) {
    let h = size / 2.0;
    match p {
        Preset::Square => (ShapeKind::Rect, vec![[cx - h, cy - h], [cx + h, cy + h]]),
        Preset::Rounded => (
            ShapeKind::Polygon,
            rounded_rect(cx - h, cy - h, size, size, size * 0.18),
        ),
        Preset::Pill => (
            ShapeKind::Polygon,
            rounded_rect(cx - h, cy - h * 0.55, size, size * 0.55, size),
        ),
        Preset::Circle => (ShapeKind::Ellipse, vec![[cx - h, cy - h], [cx + h, cy + h]]),
        Preset::Diamond => (ShapeKind::Polygon, regular(cx, cy, h, 4, -PI / 2.0)),
        Preset::Triangle => (
            ShapeKind::Polygon,
            vec![[cx, cy - h], [cx + h, cy + h * 0.8], [cx - h, cy + h * 0.8]],
        ),
        Preset::Hexagon => (ShapeKind::Polygon, regular(cx, cy, h, 6, -PI / 2.0)),
        Preset::Cloud => {
            // Bumpy outline: a wide ellipse modulated by 9 bumps.
            let pts = (0..144)
                .map(|i| {
                    let a = i as f32 / 144.0 * TAU;
                    let bump = 1.0 + 0.09 * ((a * 9.0).sin()).abs();
                    [cx + h * bump * a.cos(), cy + h * 0.62 * bump * a.sin()]
                })
                .collect();
            (ShapeKind::Polygon, pts)
        }
        Preset::Burst => (ShapeKind::Polygon, star(cx, cy, h, h * 0.78, 14)),
        Preset::Star => (ShapeKind::Polygon, star(cx, cy, h, h * 0.42, 5)),
        Preset::Heart => {
            let pts = (0..96)
                .map(|i| {
                    let t = i as f32 / 96.0 * TAU;
                    let x = 16.0 * t.sin().powi(3);
                    let y = 13.0 * t.cos()
                        - 5.0 * (2.0 * t).cos()
                        - 2.0 * (3.0 * t).cos()
                        - (4.0 * t).cos();
                    [cx + x / 17.0 * h, cy - y / 17.0 * h - h * 0.1]
                })
                .collect();
            (ShapeKind::Polygon, pts)
        }
        Preset::Arrow => (
            ShapeKind::Arrow,
            vec![[cx - h, cy + h * 0.5], [cx + h, cy - h * 0.5]],
        ),
    }
}

/// Builds a filled preset shape item.
pub fn preset_item(p: Preset, cx: f32, cy: f32, size: f32, color: Rgba) -> Item {
    let (kind, points) = preset_shape(p, cx, cy, size);
    let mut sh = Shape::new(kind, points, color, if p == Preset::Arrow { 3.0 } else { 1.5 });
    if p != Preset::Arrow {
        sh.fill = Some(color);
        sh.solid = true;
    }
    Item::new(Content::Shape(sh))
}

fn text_item(x: f32, y: f32, w: f32, text: &str, size: f32, color: Rgba) -> Item {
    Item::new(Content::Text(TextBox {
        x,
        y,
        w,
        text: text.into(),
        size,
        color,
    }))
}

fn stroke_item(pts: &[[f32; 2]], color: Rgba, width: f32) -> Item {
    Item::new(Content::Stroke(Stroke {
        pen: PenKind::Fineliner,
        color,
        width,
        points: pts
            .iter()
            .map(|p| InkPoint {
                x: p[0],
                y: p[1],
                p: 0.6,
            })
            .collect(),
    }))
}

/// The sample sticker pack: quiz "?" bubbles, stars, arrows, checks and labels.
pub fn sample_stickers() -> Vec<Sticker> {
    let c = |h: &str| Rgba::from_hex(h).unwrap_or(Rgba::BLACK);
    let mut out: Vec<Vec<Item>> = Vec::new();
    // "?" quiz bubble
    out.push(vec![
        preset_item(Preset::Circle, 30.0, 30.0, 56.0, c("#7b3fe4")),
        text_item(17.0, 6.0, 30.0, "?", 34.0, Rgba::WHITE),
    ]);
    // "!" important bubble
    out.push(vec![
        preset_item(Preset::Circle, 30.0, 30.0, 56.0, c("#ff3b30")),
        text_item(22.0, 6.0, 30.0, "!", 34.0, Rgba::WHITE),
    ]);
    out.push(vec![preset_item(Preset::Star, 30.0, 30.0, 60.0, c("#ffcc00"))]);
    out.push(vec![preset_item(Preset::Heart, 30.0, 30.0, 58.0, c("#ff6f91"))]);
    out.push(vec![stroke_item(
        &[[8.0, 30.0], [24.0, 46.0], [54.0, 12.0]],
        c("#34c759"),
        6.0,
    )]);
    out.push(vec![stroke_item(
        &[[10.0, 10.0], [50.0, 50.0]],
        c("#ff3b30"),
        6.0,
    ), stroke_item(&[[50.0, 10.0], [10.0, 50.0]], c("#ff3b30"), 6.0)]);
    out.push(vec![{
        let mut sh = Shape::new(
            ShapeKind::Arrow,
            vec![[6.0, 44.0], [58.0, 14.0]],
            c("#0a84ff"),
            4.0,
        );
        sh.fill = None;
        Item::new(Content::Shape(sh))
    }]);
    // Light bulb idea
    out.push(vec![
        preset_item(Preset::Circle, 30.0, 24.0, 40.0, c("#ffd60a")),
        preset_item(Preset::Square, 30.0, 50.0, 14.0, c("#8e8e93")),
    ]);
    for (label, color) in [
        ("Important", "#ff3b30"),
        ("Exam", "#7b3fe4"),
        ("Review", "#0a84ff"),
        ("Done", "#34c759"),
    ] {
        let w = 18.0 + label.len() as f32 * 9.5;
        let mut items = vec![{
            let (_, pts) = preset_shape(Preset::Pill, w / 2.0, 18.0, w);
            let mut sh = Shape::new(ShapeKind::Polygon, pts, c(color), 1.0);
            sh.fill = Some(c(color));
            sh.solid = true;
            Item::new(Content::Shape(sh))
        }];
        items.push(text_item(10.0, 6.0, w, label, 16.0, Rgba::WHITE));
        out.push(items);
    }
    out.into_iter()
        .map(|items| Sticker {
            id: new_id(),
            items,
            used: 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_points() {
        for p in Preset::ALL {
            let (_, pts) = preset_shape(p, 100.0, 100.0, 80.0);
            assert!(pts.len() >= 2, "{p:?}");
            assert!(pts.iter().all(|q| q[0].is_finite() && q[1].is_finite()));
        }
        assert!(sample_stickers().len() >= 8);
    }
}
