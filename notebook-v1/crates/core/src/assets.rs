//! Pictures (imported photos and PDF pages). They are stored once as JPEG bytes
//! and referenced by id from pages and image items.

use crate::model::Id;
use std::sync::Arc;

/// Decoded picture, straight RGBA.
#[derive(Clone, Debug)]
pub struct RgbaImage {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

/// Anything that can hand out decoded pictures (a cache, or the store directly).
pub trait AssetSource {
    fn image(&self, id: Id) -> Option<Arc<RgbaImage>>;
}

/// No pictures available: images render as light placeholders.
pub struct NoAssets;

impl AssetSource for NoAssets {
    fn image(&self, _id: Id) -> Option<Arc<RgbaImage>> {
        None
    }
}

/// Width and height of a JPEG without decoding the pixels.
pub fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut d = jpeg_decoder::Decoder::new(bytes);
    d.read_info().ok()?;
    let info = d.info()?;
    Some((info.width as u32, info.height as u32))
}

/// Number of colour components (1 = grey, 3 = RGB) – PDF needs to know.
pub fn jpeg_components(bytes: &[u8]) -> Option<u8> {
    let mut d = jpeg_decoder::Decoder::new(bytes);
    d.read_info().ok()?;
    Some(match d.info()?.pixel_format {
        jpeg_decoder::PixelFormat::L8 | jpeg_decoder::PixelFormat::L16 => 1,
        jpeg_decoder::PixelFormat::CMYK32 => 4,
        jpeg_decoder::PixelFormat::RGB24 => 3,
    })
}

pub fn decode_jpeg(bytes: &[u8]) -> Option<RgbaImage> {
    let mut d = jpeg_decoder::Decoder::new(bytes);
    let data = d.decode().ok()?;
    let info = d.info()?;
    let (w, h) = (info.width as u32, info.height as u32);
    let n = (w * h) as usize;
    let mut px = Vec::with_capacity(n * 4);
    match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            for c in data.chunks_exact(3).take(n) {
                px.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        jpeg_decoder::PixelFormat::L8 => {
            for &g in data.iter().take(n) {
                px.extend_from_slice(&[g, g, g, 255]);
            }
        }
        jpeg_decoder::PixelFormat::L16 => {
            for c in data.chunks_exact(2).take(n) {
                px.extend_from_slice(&[c[0], c[0], c[0], 255]);
            }
        }
        jpeg_decoder::PixelFormat::CMYK32 => {
            for c in data.chunks_exact(4).take(n) {
                let k = 255 - c[3] as u32;
                let f = |v: u8| ((255 - v as u32) * k / 255) as u8;
                px.extend_from_slice(&[f(c[0]), f(c[1]), f(c[2]), 255]);
            }
        }
    }
    if px.len() != n * 4 {
        return None;
    }
    Some(RgbaImage { w, h, px })
}

/// Shrinks an image so its longer side is at most `max` pixels (box filter).
pub fn downscale(img: &RgbaImage, max: u32) -> RgbaImage {
    let long = img.w.max(img.h);
    if long <= max {
        return img.clone();
    }
    let f = long.div_ceil(max);
    let (w, h) = (img.w / f, img.h / f);
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for dy in 0..f {
                for dx in 0..f {
                    let i = (((y * f + dy) * img.w + x * f + dx) * 4) as usize;
                    for k in 0..4 {
                        acc[k] += img.px[i + k] as u32;
                    }
                }
            }
            let o = ((y * w + x) * 4) as usize;
            for k in 0..4 {
                px[o + k] = (acc[k] / (f * f)) as u8;
            }
        }
    }
    RgbaImage { w, h, px }
}
