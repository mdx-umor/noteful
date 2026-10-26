//! Text layout shared by every renderer, so wrapping is identical on screen,
//! in thumbnails and in exported PDFs.

use ab_glyph::{Font, FontRef, ScaleFont};
use std::sync::OnceLock;

pub const LINE_HEIGHT: f32 = 1.35;

pub fn font() -> &'static FontRef<'static> {
    static FONT: OnceLock<FontRef<'static>> = OnceLock::new();
    FONT.get_or_init(|| {
        FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).expect("bundled font")
    })
}

pub fn text_width(s: &str, size: f32) -> f32 {
    let f = font().as_scaled(size);
    let mut w = 0.0;
    let mut prev = None;
    for ch in s.chars() {
        let id = f.glyph_id(ch);
        if let Some(p) = prev {
            w += f.kern(p, id);
        }
        w += f.h_advance(id);
        prev = Some(id);
    }
    w
}

/// Word-wraps `text` to `max_w`. Explicit newlines always break; words wider
/// than the box are broken by characters.
pub fn wrap(text: &str, size: f32, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in split_keep_spaces(para) {
            let candidate = format!("{line}{word}");
            if line.is_empty() || text_width(candidate.trim_end(), size) <= max_w {
                line = candidate;
            } else {
                lines.push(line.trim_end().to_string());
                line = word.trim_start().to_string();
            }
            while text_width(line.trim_end(), size) > max_w && line.chars().count() > 1 {
                let mut cut = 0;
                let mut acc = String::new();
                for (i, ch) in line.char_indices() {
                    acc.push(ch);
                    if text_width(&acc, size) > max_w {
                        cut = if i == 0 { ch.len_utf8() } else { i };
                        break;
                    }
                }
                if cut == 0 || cut >= line.len() {
                    break;
                }
                lines.push(line[..cut].to_string());
                line = line[cut..].to_string();
            }
        }
        lines.push(line.trim_end().to_string());
    }
    lines
}

fn split_keep_spaces(s: &str) -> Vec<&str> {
    // Splits into words that carry their trailing whitespace.
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = false;
    for (i, ch) in s.char_indices() {
        let space = ch.is_whitespace();
        if in_space && !space {
            out.push(&s[start..i]);
            start = i;
        }
        in_space = space;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

pub fn text_height(text: &str, size: f32, w: f32) -> f32 {
    wrap(text, size, w).len().max(1) as f32 * size * LINE_HEIGHT
}

/// Ascent of the font at `size`, used to place the first baseline.
pub fn ascent(size: f32) -> f32 {
    font().as_scaled(size).ascent()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_words() {
        let lines = wrap("hello world this is a test", 20.0, 80.0);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(text_width(l, 20.0) <= 80.0 + 0.01, "{l}");
        }
        assert_eq!(wrap("a\n\nb", 12.0, 100.0), vec!["a", "", "b"]);
    }

    #[test]
    fn breaks_long_words() {
        let lines = wrap("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 20.0, 60.0);
        assert!(lines.len() > 2);
    }
}
