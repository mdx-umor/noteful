//! Draws the 1024×1024 app icon with the notebook's own ink engine.
//! `cargo run -p notebook_core --example app_icon -- out.png`
use notebook_core::*;
use tiny_skia::{
    Color, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, Rect, SpreadMode,
    Transform,
};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "icon-1024.png".into());
    let size = 1024.0;
    let mut pix = Pixmap::new(1024, 1024).expect("pixmap");
    let mut bg = Paint::default();
    bg.shader = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(size, size),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(0x6a, 0x3c, 0xf0, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0x0a, 0x84, 0xff, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .expect("gradient");
    pix.fill_rect(
        Rect::from_xywh(0.0, 0.0, size, size).expect("rect"),
        &bg,
        Transform::identity(),
        None,
    );

    // A tilted sheet of lined paper.
    let mut page = Page::new(Template::Lined, Paper::White);
    page.w = 520.0;
    page.h = 660.0;
    let ink: Vec<InkPoint> = (0..=160)
        .map(|i| {
            let s = i as f32 / 160.0 * 4.0;
            InkPoint {
                x: 80.0 + s * 95.0 + (s * std::f32::consts::TAU).sin() * 26.0,
                y: 330.0 - (s * std::f32::consts::TAU).cos() * 34.0,
                p: 0.35 + 0.6 * (s * 1.7).sin().abs(),
            }
        })
        .collect();
    page.items
        .push(std::sync::Arc::new(Item::new(Content::Stroke(Stroke {
            pen: PenKind::Highlighter,
            color: Rgba::rgb(255, 214, 10),
            width: 34.0,
            points: (0..=20)
                .map(|i| InkPoint {
                    x: 80.0 + i as f32 * 19.0,
                    y: 470.0,
                    p: 0.5,
                })
                .collect(),
        }))));
    page.items
        .push(std::sync::Arc::new(Item::new(Content::Stroke(Stroke {
            pen: PenKind::Fountain,
            color: Rgba::rgb(0x1d, 0x1d, 0x1f),
            width: 14.0,
            points: ink,
        }))));
    let sheet = render::render_page(&page, 520);
    let mut shadow = Paint::default();
    shadow.set_color_rgba8(0, 0, 0, 60);
    let tf = Transform::from_rotate_at(-6.0, 512.0, 512.0).pre_translate(252.0, 196.0);
    let p = PathBuilder::from_rect(Rect::from_xywh(10.0, 18.0, 520.0, 660.0).expect("rect"));
    pix.fill_path(&p, &shadow, tiny_skia::FillRule::Winding, tf, None);
    pix.draw_pixmap(
        0,
        0,
        sheet.as_ref(),
        &tiny_skia::PixmapPaint {
            quality: tiny_skia::FilterQuality::Bicubic,
            ..Default::default()
        },
        tf,
        None,
    );
    pix.save_png(&out).expect("save");
    println!("wrote {out}");
}
