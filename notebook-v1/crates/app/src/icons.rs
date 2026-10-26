//! Vector icons drawn with the painter, so they stay crisp at any scale.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // a complete set, not all used yet
pub enum Icon {
    Back,
    Pen,
    Highlighter,
    Eraser,
    Lasso,
    Hand,
    Laser,
    Shapes,
    Text,
    Plus,
    PlusCircle,
    Undo,
    Redo,
    Grid,
    Sidebar,
    Fullscreen,
    More,
    Search,
    List,
    Sort,
    Select,
    Quick,
    Notebook,
    Untagged,
    Unfiled,
    Trash,
    Pin,
    Folder,
    Hash,
    Ruler,
    ZoomIn,
    ZoomOut,
    Close,
    Check,
    Bookmark,
    Copy,
    Cut,
    Duplicate,
    Paste,
    Export,
    Gear,
    Chevron,
    ChevronRight,
    Tape,
    Sticker,
}

fn poly(p: &Painter, pts: &[Pos2], s: Stroke) {
    p.add(Shape::line(pts.to_vec(), s));
}

/// Draws `icon` centred in `rect` using a 24×24 design grid.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let k = rect.width().min(rect.height()) / 24.0;
    let o = rect.center() - vec2(12.0, 12.0) * k;
    let q = |x: f32, y: f32| o + vec2(x, y) * k;
    let s = Stroke::new(1.7 * k, color);
    let line = |a: (f32, f32), b: (f32, f32)| p.line_segment([q(a.0, a.1), q(b.0, b.1)], s);
    let circle = |c: (f32, f32), r: f32| p.circle_stroke(q(c.0, c.1), r * k, s);
    let arc = |c: (f32, f32), r: f32, a0: f32, a1: f32| {
        let n = 20;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let t = a0 + (a1 - a0) * i as f32 / n as f32;
                q(c.0 + r * t.cos(), c.1 + r * t.sin())
            })
            .collect();
        poly(p, &pts, s);
    };
    use std::f32::consts::PI;
    match icon {
        Icon::Back => poly(p, &[q(15.0, 5.0), q(8.0, 12.0), q(15.0, 19.0)], s),
        Icon::Tape => {
            // A strip of washi tape at an angle, with stripes.
            let pts = [q(3.0, 15.0), q(15.0, 3.0), q(21.0, 9.0), q(9.0, 21.0), q(3.0, 15.0)];
            p.add(Shape::convex_polygon(
                pts[..4].to_vec(),
                color.gamma_multiply(0.25),
                Stroke::NONE,
            ));
            poly(p, &pts, s);
            for k in 0..3 {
                let o = k as f32 * 4.0;
                line((8.0 + o, 10.0 - o), (13.0 + o, 15.0 - o));
            }
        }
        Icon::Sticker => {
            // Star with a peeled corner.
            let star: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -PI / 2.0 + i as f32 * PI / 5.0;
                    let r = if i % 2 == 0 { 9.0 } else { 4.0 };
                    q(12.0 + r * a.cos(), 12.5 + r * a.sin())
                })
                .collect();
            let mut closed = star.clone();
            closed.push(star[0]);
            poly(p, &closed, s);
        }
        Icon::Chevron => poly(p, &[q(7.0, 10.0), q(12.0, 15.0), q(17.0, 10.0)], s),
        Icon::ChevronRight => poly(p, &[q(10.0, 7.0), q(15.0, 12.0), q(10.0, 17.0)], s),
        Icon::Pen => {
            p.add(Shape::convex_polygon(
                vec![
                    q(4.0, 20.0),
                    q(5.5, 15.0),
                    q(16.0, 4.5),
                    q(19.5, 8.0),
                    q(9.0, 18.5),
                ],
                color.gamma_multiply(0.25),
                s,
            ));
            line((14.0, 6.5), (17.5, 10.0));
        }
        Icon::Highlighter => {
            poly(
                p,
                &[
                    q(9.0, 13.0),
                    q(16.5, 5.5),
                    q(20.5, 9.5),
                    q(13.0, 17.0),
                    q(9.0, 13.0),
                ],
                s,
            );
            p.add(Shape::convex_polygon(
                vec![q(9.0, 13.0), q(13.0, 17.0), q(9.0, 19.0), q(7.0, 15.0)],
                color,
                Stroke::NONE,
            ));
            line((4.0, 21.0), (20.0, 21.0));
        }
        Icon::Eraser => {
            poly(
                p,
                &[
                    q(4.0, 15.5),
                    q(13.5, 6.0),
                    q(19.5, 12.0),
                    q(11.0, 20.5),
                    q(8.5, 20.5),
                    q(4.0, 15.5),
                ],
                s,
            );
            line((8.5, 11.0), (14.5, 17.0));
            line((11.0, 20.5), (20.0, 20.5));
        }
        Icon::Lasso => {
            arc((12.5, 10.0), 8.5, PI * 0.75, PI * 2.6);
            arc((12.5, 10.0), 8.5, PI * 2.6, PI * 2.75);
            circle((6.0, 16.5), 2.0);
            poly(p, &[q(6.0, 18.5), q(5.0, 21.5)], s);
        }
        Icon::Hand => {
            for (x, top) in [(8.0, 8.0), (11.0, 5.0), (14.0, 5.5), (17.0, 8.0)] {
                line((x, top), (x, 13.0));
            }
            poly(
                p,
                &[
                    q(8.0, 13.0),
                    q(8.0, 16.0),
                    q(6.0, 13.5),
                    q(4.5, 14.5),
                    q(8.0, 20.0),
                    q(15.0, 21.0),
                    q(17.0, 17.0),
                    q(17.0, 13.0),
                ],
                s,
            );
        }
        Icon::Laser => {
            p.circle_filled(q(12.0, 12.0), 3.2 * k, color);
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                line(
                    (12.0 + 6.0 * a.cos(), 12.0 + 6.0 * a.sin()),
                    (12.0 + 9.0 * a.cos(), 12.0 + 9.0 * a.sin()),
                );
            }
        }
        Icon::Shapes => {
            p.rect_stroke(
                Rect::from_min_max(q(3.5, 11.5), q(12.5, 20.5)),
                1.5 * k,
                s,
                egui::StrokeKind::Middle,
            );
            circle((16.0, 8.0), 5.0);
        }
        Icon::Text => {
            line((5.0, 6.0), (19.0, 6.0));
            line((12.0, 6.0), (12.0, 19.0));
            line((9.0, 19.0), (15.0, 19.0));
        }
        Icon::Plus => {
            line((12.0, 5.0), (12.0, 19.0));
            line((5.0, 12.0), (19.0, 12.0));
        }
        Icon::PlusCircle => {
            circle((12.0, 12.0), 9.0);
            line((12.0, 7.5), (12.0, 16.5));
            line((7.5, 12.0), (16.5, 12.0));
        }
        Icon::Undo | Icon::Redo => {
            let f = |x: f32| if icon == Icon::Undo { x } else { 24.0 - x };
            arc(
                (f(13.0), 13.5),
                6.0,
                if icon == Icon::Undo {
                    -PI * 0.5
                } else {
                    PI * 1.5
                },
                PI * 0.5,
            );
            line((f(13.0), 7.5), (f(5.5), 7.5));
            poly(p, &[q(f(8.5), 4.5), q(f(5.5), 7.5), q(f(8.5), 10.5)], s);
            line((f(13.0), 19.5), (f(9.0), 19.5));
        }
        Icon::Grid => {
            for (x, y) in [(4.0, 4.0), (13.5, 4.0), (4.0, 13.5), (13.5, 13.5)] {
                p.rect_stroke(
                    Rect::from_min_size(q(x, y), vec2(6.5, 6.5) * k),
                    1.5 * k,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::Sidebar => {
            p.rect_stroke(
                Rect::from_min_max(q(3.0, 5.0), q(21.0, 19.0)),
                2.0 * k,
                s,
                egui::StrokeKind::Middle,
            );
            line((15.0, 5.0), (15.0, 19.0));
        }
        Icon::Fullscreen => {
            p.rect_stroke(
                Rect::from_min_max(q(3.0, 6.0), q(21.0, 18.0)),
                2.0 * k,
                s,
                egui::StrokeKind::Middle,
            );
            p.rect_filled(
                Rect::from_min_max(q(5.5, 8.5), q(18.5, 15.5)),
                1.0 * k,
                color,
            );
        }
        Icon::More => {
            for x in [5.0, 12.0, 19.0] {
                p.circle_filled(q(x, 12.0), 1.7 * k, color);
            }
        }
        Icon::Search => {
            circle((10.5, 10.5), 6.5);
            line((15.5, 15.5), (20.5, 20.5));
        }
        Icon::List => {
            for y in [6.0, 12.0, 18.0] {
                p.circle_filled(q(4.5, y), 1.2 * k, color);
                line((8.5, y), (20.0, y));
            }
        }
        Icon::Sort => {
            line((8.0, 4.0), (8.0, 20.0));
            poly(p, &[q(4.5, 7.5), q(8.0, 4.0), q(11.5, 7.5)], s);
            line((16.0, 4.0), (16.0, 20.0));
            poly(p, &[q(12.5, 16.5), q(16.0, 20.0), q(19.5, 16.5)], s);
        }
        Icon::Select | Icon::Check => {
            if icon == Icon::Select {
                p.rect_stroke(
                    Rect::from_min_max(q(3.5, 3.5), q(20.5, 20.5)),
                    3.0 * k,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
            poly(p, &[q(7.5, 12.0), q(10.5, 15.0), q(16.5, 8.5)], s);
        }
        Icon::Quick => {
            poly(
                p,
                &[
                    q(3.0, 20.0),
                    q(6.0, 13.0),
                    q(15.5, 3.5),
                    q(20.5, 8.5),
                    q(11.0, 18.0),
                    q(3.0, 20.0),
                ],
                s,
            );
            poly(
                p,
                &[q(13.0, 21.0), q(16.0, 18.5), q(18.0, 21.0), q(21.0, 18.0)],
                s,
            );
        }
        Icon::Notebook => {
            p.rect_stroke(
                Rect::from_min_max(q(5.0, 3.0), q(19.0, 21.0)),
                2.0 * k,
                s,
                egui::StrokeKind::Middle,
            );
            for y in [7.0, 12.0, 17.0] {
                line((3.0, y), (7.0, y));
            }
            line((11.0, 15.0), (16.0, 10.0));
        }
        Icon::Untagged => {
            for i in 0..12 {
                let a0 = i as f32 * PI / 6.0;
                arc((12.0, 12.0), 8.0, a0, a0 + PI / 10.0);
            }
        }
        Icon::Unfiled | Icon::Folder => {
            poly(
                p,
                &[
                    q(3.0, 7.0),
                    q(3.0, 19.0),
                    q(21.0, 19.0),
                    q(21.0, 9.0),
                    q(11.0, 9.0),
                    q(9.0, 6.0),
                    q(3.0, 6.0),
                    q(3.0, 7.0),
                ],
                s,
            );
            if icon == Icon::Unfiled {
                p.circle_filled(q(17.0, 6.0), 3.2 * k, color);
            }
        }
        Icon::Trash => {
            line((4.0, 6.5), (20.0, 6.5));
            poly(
                p,
                &[q(9.0, 6.5), q(9.0, 4.0), q(15.0, 4.0), q(15.0, 6.5)],
                s,
            );
            poly(
                p,
                &[q(6.0, 6.5), q(7.0, 20.0), q(17.0, 20.0), q(18.0, 6.5)],
                s,
            );
            line((10.5, 10.0), (10.5, 16.5));
            line((13.5, 10.0), (13.5, 16.5));
        }
        Icon::Pin => {
            poly(
                p,
                &[
                    q(8.0, 4.0),
                    q(16.0, 4.0),
                    q(15.0, 10.0),
                    q(18.0, 14.0),
                    q(6.0, 14.0),
                    q(9.0, 10.0),
                    q(8.0, 4.0),
                ],
                s,
            );
            line((12.0, 14.0), (12.0, 21.0));
        }
        Icon::Hash => {
            p.circle_filled(q(12.0, 12.0), 10.0 * k, color);
            let light = color.r() as u32 + color.g() as u32 + color.b() as u32 > 380;
            let s2 = Stroke::new(
                1.6 * k,
                if light {
                    Color32::from_black_alpha(220)
                } else {
                    Color32::WHITE
                },
            );
            for (a, b) in [
                ((7.0, 10.0), (17.0, 10.0)),
                ((7.0, 14.0), (17.0, 14.0)),
                ((10.5, 6.5), (9.5, 17.5)),
                ((14.5, 6.5), (13.5, 17.5)),
            ] {
                p.line_segment([q(a.0, a.1), q(b.0, b.1)], s2);
            }
        }
        Icon::Ruler => {
            p.rect_stroke(
                Rect::from_min_max(q(2.0, 8.0), q(22.0, 16.0)),
                1.5 * k,
                s,
                egui::StrokeKind::Middle,
            );
            for (i, x) in [5.5, 9.0, 12.5, 16.0, 19.5].iter().enumerate() {
                line((*x, 8.0), (*x, if i % 2 == 0 { 12.0 } else { 10.5 }));
            }
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            circle((10.5, 10.5), 6.5);
            line((15.5, 15.5), (20.5, 20.5));
            line((7.5, 10.5), (13.5, 10.5));
            if icon == Icon::ZoomIn {
                line((10.5, 7.5), (10.5, 13.5));
            }
        }
        Icon::Close => {
            line((6.0, 6.0), (18.0, 18.0));
            line((18.0, 6.0), (6.0, 18.0));
        }
        Icon::Bookmark => poly(
            p,
            &[
                q(6.0, 3.5),
                q(18.0, 3.5),
                q(18.0, 20.5),
                q(12.0, 16.0),
                q(6.0, 20.5),
                q(6.0, 3.5),
            ],
            s,
        ),
        Icon::Copy | Icon::Duplicate => {
            p.rect_stroke(
                Rect::from_min_max(q(8.5, 8.5), q(20.0, 20.0)),
                2.0 * k,
                s,
                egui::StrokeKind::Middle,
            );
            poly(
                p,
                &[
                    q(15.5, 8.5),
                    q(15.5, 4.0),
                    q(4.0, 4.0),
                    q(4.0, 15.5),
                    q(8.5, 15.5),
                ],
                s,
            );
            if icon == Icon::Duplicate {
                line((14.25, 11.5), (14.25, 17.0));
                line((11.5, 14.25), (17.0, 14.25));
            }
        }
        Icon::Cut => {
            circle((6.5, 17.5), 2.8);
            circle((17.5, 17.5), 2.8);
            line((8.5, 15.5), (17.0, 4.0));
            line((15.5, 15.5), (7.0, 4.0));
        }
        Icon::Paste => {
            p.rect_stroke(
                Rect::from_min_max(q(5.0, 5.0), q(19.0, 21.0)),
                2.0 * k,
                s,
                egui::StrokeKind::Middle,
            );
            p.rect_filled(
                Rect::from_min_max(q(9.0, 3.0), q(15.0, 7.0)),
                1.0 * k,
                color,
            );
        }
        Icon::Export => {
            line((12.0, 3.5), (12.0, 15.0));
            poly(p, &[q(8.0, 7.5), q(12.0, 3.5), q(16.0, 7.5)], s);
            poly(
                p,
                &[
                    q(8.0, 11.0),
                    q(5.0, 11.0),
                    q(5.0, 20.5),
                    q(19.0, 20.5),
                    q(19.0, 11.0),
                    q(16.0, 11.0),
                ],
                s,
            );
        }
        Icon::Gear => {
            circle((12.0, 12.0), 3.0);
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                line(
                    (12.0 + 6.0 * a.cos(), 12.0 + 6.0 * a.sin()),
                    (12.0 + 8.5 * a.cos(), 12.0 + 8.5 * a.sin()),
                );
            }
            circle((12.0, 12.0), 6.0);
        }
    }
    let _ = pos2(0.0, 0.0);
}

/// A square icon button. `active` shows the selected state.
pub fn button(ui: &mut egui::Ui, icon: Icon, active: bool, tip: &str) -> egui::Response {
    icon_button(ui, icon, active, tip, 40.0)
}

pub fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    active: bool,
    tip: &str,
    size: f32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    let v = ui.visuals();
    let accent = v.selection.bg_fill;
    if active {
        ui.painter()
            .rect_filled(rect.shrink(2.0), 10.0, accent.gamma_multiply(0.22));
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect.shrink(2.0), 10.0, v.widgets.hovered.bg_fill);
    }
    let color = if active {
        accent
    } else if resp.enabled() {
        v.text_color()
    } else {
        v.weak_text_color()
    };
    paint(ui.painter(), rect.shrink(size * 0.22), icon, color);
    resp.on_hover_text(tip)
}
