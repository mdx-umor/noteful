//! Renders a sample page with every pen, shapes and text to PNG and PDF.
//! `cargo run -p notebook_core --example sample_page -- out_dir`
use notebook_core::*;
use std::sync::Arc;

fn wave(y: f32, pen: PenKind, color: Rgba, width: f32, pressure: impl Fn(f32) -> f32) -> Item {
    let points = (0..=160)
        .map(|i| {
            let t = i as f32 / 160.0;
            InkPoint {
                x: 60.0 + t * 470.0,
                y: y + (t * 12.0).sin() * 18.0 + (t * 31.0).cos() * 4.0,
                p: pressure(t),
            }
        })
        .collect();
    Item::new(Content::Stroke(Stroke {
        pen,
        color,
        width,
        points,
    }))
}

fn handwriting(x0: f32, y0: f32) -> Item {
    // A looping cursive-like line to show fountain thick/thin.
    let points = (0..=300)
        .map(|i| {
            let t = i as f32 / 300.0 * 10.0;
            InkPoint {
                x: x0 + t * 42.0 + (t * std::f32::consts::TAU).sin() * 14.0,
                y: y0 - (t * std::f32::consts::TAU).cos() * 16.0,
                p: 0.45 + 0.4 * (t * 3.1).sin().abs(),
            }
        })
        .collect();
    Item::new(Content::Stroke(Stroke {
        pen: PenKind::Fountain,
        color: Rgba::rgb(20, 40, 120),
        width: mm(0.8),
        points,
    }))
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut page = Page::new(Template::Lined, Paper::Cream);
    let items = vec![
        wave(150.0, PenKind::Highlighter, Rgba::rgb(255, 214, 10), mm(5.0), |_| 0.5),
        wave(110.0, PenKind::Fineliner, Rgba::BLACK, mm(0.8), |_| 0.5),
        wave(150.0, PenKind::Fountain, Rgba::rgb(30, 91, 216), mm(1.2), |t| 0.2 + 0.8 * (t * 9.0).sin().abs()),
        wave(200.0, PenKind::Brush, Rgba::rgb(217, 48, 37), mm(1.9), |t| (t * std::f32::consts::PI).sin()),
        handwriting(60.0, 280.0),
        Item::new(Content::Shape(Shape { kind: ShapeKind::Rect, points: vec![[60.0, 340.0], [220.0, 440.0]], color: Rgba::rgb(24, 128, 56), width: 2.0, fill: Some(Rgba::rgb(24, 128, 56)), dashed: false, solid: false, dotted: false })),
        Item::new(Content::Shape(Shape { kind: ShapeKind::Ellipse, points: vec![[250.0, 340.0], [380.0, 440.0]], color: Rgba::rgb(123, 63, 228), width: 2.5, fill: None, dashed: true, solid: false, dotted: false })),
        Item::new(Content::Shape(Shape { kind: ShapeKind::Arrow, points: vec![[400.0, 440.0], [530.0, 350.0]], color: Rgba::BLACK, width: 2.0, fill: None, dashed: false, solid: false, dotted: false })),
        Item::new(Content::Text(TextBox { x: 60.0, y: 470.0, w: 470.0, text: "Notebook v1 — typed text wraps the same way on screen, in thumbnails and in exported PDFs.".into(), size: 18.0, color: Rgba::rgb(29, 29, 31) })),
    ];
    page.items = items.into_iter().map(Arc::new).collect();
    std::fs::write(
        format!("{out}/sample_page.png"),
        render::render_png(&page, 1190),
    )
    .unwrap();
    let mut doc = Document::new(1, Template::Lined, Paper::Cream);
    doc.pages[0] = page;
    for t in [
        Template::Grid,
        Template::Dotted,
        Template::Cornell,
        Template::Todo,
        Template::Music,
        Template::Storyboard,
    ] {
        doc.pages.push(Page::new(t, Paper::White));
    }
    doc.pages.push(Page::new(Template::Lined, Paper::Night));
    std::fs::write(format!("{out}/sample.pdf"), pdf::export(&doc, "Sample")).unwrap();
    for (i, p) in doc.pages.iter().enumerate().skip(1) {
        std::fs::write(
            format!("{out}/template_{i}.png"),
            render::render_png(p, 300),
        )
        .unwrap();
    }
    println!("wrote {out}");
}
