//! Shared egui helpers used by components that render host-supplied assets.

use egui::{Context, Id, TextureHandle};
use std::path::Path;

/// Maximum icon file size we're willing to read. Protects against accidentally
/// pointing at a large asset and allocating huge amounts of memory.
const MAX_ICON_SIZE_BYTES: u64 = 5_000_000;

/// Maximum decoded pixel count (width * height) we're willing to allocate.
/// The compressed-size cap doesn't bound the decoded RGBA buffer (a small file
/// can decode into huge dimensions), so guard the pixel count too. 4096*4096
/// pixels = 64 MiB at 4 bytes/pixel, a comfortable ceiling for icons.
const MAX_ICON_PIXELS: u64 = 4096 * 4096;

/// Decode a PNG/ICO from `path` and upload it to the GPU as an egui texture.
///
/// The `TextureHandle` is cached in egui memory keyed by `(name, path)`, so the
/// file is read and decoded only once per session. Returns `None` if the file
/// is missing, exceeds the file-size or decoded-pixel caps, or fails to decode.
pub fn load_icon_texture(ctx: &Context, path: &Path, name: &str) -> Option<TextureHandle> {
    let key = Id::new((name, path));
    if let Some(cached) = ctx.memory(|mem| mem.data.get_temp::<TextureHandle>(key)) {
        return Some(cached);
    }
    let texture = decode_image_to_texture(ctx, path)?;
    ctx.memory_mut(|mem| mem.data.insert_temp(key, texture.clone()));
    Some(texture)
}

fn decode_image_to_texture(ctx: &Context, path: &Path) -> Option<TextureHandle> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_ICON_SIZE_BYTES => return None,
        Err(_) => return None,
        _ => {}
    }
    let bytes = std::fs::read(path).ok()?;

    // Guard the decoded dimensions before fully decoding: a small compressed
    // file can still expand into an enormous RGBA buffer (decode bomb).
    let (w, h) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    if u64::from(w) * u64::from(h) > MAX_ICON_PIXELS {
        return None;
    }

    let img = image::load_from_memory(&bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    Some(ctx.load_texture(
        path.to_string_lossy(),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}
