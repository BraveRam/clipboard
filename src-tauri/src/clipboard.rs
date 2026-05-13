use crate::commands::CapturedEvent;
use crate::db::Repo;
use anyhow::{Context, Result};
use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{ImageBuffer, ImageReader, Rgba};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Runtime};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const THUMB_TARGET_WIDTH: u32 = 320;

/// Tracks content we just placed on the clipboard ourselves so the next
/// poll cycle does not re-capture our own write.
pub struct WriteGuard {
    inner: Mutex<Option<String>>,
}

impl WriteGuard {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub fn arm(&self, hash: String) {
        *self.inner.lock() = Some(hash);
    }

    pub fn should_skip(&self, hash: &str) -> bool {
        let mut guard = self.inner.lock();
        if guard.as_deref() == Some(hash) {
            *guard = None;
            return true;
        }
        false
    }
}

/// Snapshot of the polling thread's most recent hashes so we only act on
/// real changes.
#[derive(Default)]
struct WatcherState {
    last_text_hash: Option<String>,
    last_image_hash: Option<String>,
}

pub fn spawn_watcher<R: Runtime>(
    app: AppHandle<R>,
    repo: Arc<Repo>,
    guard: Arc<WriteGuard>,
) {
    std::thread::spawn(move || {
        let mut cb = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[clipboard] arboard init failed: {e}");
                return;
            }
        };
        let mut state = WatcherState::default();
        loop {
            if let Err(e) = tick(&mut cb, &mut state, &repo, &guard, &app) {
                // Most ticks will hit harmless "no content" errors from arboard
                // depending on what's on the clipboard; only log unexpected
                // failures at debug level.
                let msg = e.to_string();
                if !is_benign(&msg) {
                    eprintln!("[clipboard] watcher tick error: {msg}");
                }
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    });
}

fn is_benign(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("contentnotavailable")
        || m.contains("empty")
        || m.contains("no content")
        || m.contains("conversionfailure")
}

fn tick<R: Runtime>(
    cb: &mut Clipboard,
    state: &mut WatcherState,
    repo: &Arc<Repo>,
    guard: &Arc<WriteGuard>,
    app: &AppHandle<R>,
) -> Result<()> {
    if let Ok(text) = cb.get_text() {
        if !text.is_empty() {
            let hash = sha256_hex(text.as_bytes());
            if state.last_text_hash.as_deref() != Some(&hash) {
                state.last_text_hash = Some(hash.clone());
                // Reset image dedup so the next image (even an identical one)
                // is treated as a fresh selection if it follows a text copy.
                state.last_image_hash = None;
                if !guard.should_skip(&hash) {
                    capture_text(app, repo, &text, &hash)?;
                }
            }
            return Ok(());
        }
    }

    if let Ok(img) = cb.get_image() {
        let hash = sha256_hex(&img.bytes);
        if state.last_image_hash.as_deref() != Some(&hash) {
            state.last_image_hash = Some(hash.clone());
            state.last_text_hash = None;
            if !guard.should_skip(&hash) {
                capture_image(app, repo, img, &hash)?;
            }
        }
    }

    Ok(())
}

fn capture_text<R: Runtime>(
    app: &AppHandle<R>,
    repo: &Arc<Repo>,
    text: &str,
    hash: &str,
) -> Result<()> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let size = text.len() as i64;
    let (id, is_new) = repo
        .upsert("text", Some(text), None, None, None, None, size, hash)
        .context("upsert text")?;
    let _ = app.emit("clipboard:entry-captured", CapturedEvent { id, is_new });
    Ok(())
}

fn capture_image<R: Runtime>(
    app: &AppHandle<R>,
    repo: &Arc<Repo>,
    img: ImageData<'_>,
    hash: &str,
) -> Result<()> {
    let width = img.width as u32;
    let height = img.height as u32;
    let raw = img.bytes.into_owned();

    let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, raw)
        .context("rgba buffer build")?;

    // Encode the canonical PNG representation for on-disk storage.
    let mut png_bytes: Vec<u8> = Vec::with_capacity(64 * 1024);
    image::DynamicImage::ImageRgba8(buffer.clone())
        .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .context("encode png")?;

    let filename = format!("{}.png", hash);
    let full_path = repo.images_dir().join(&filename);
    if !full_path.exists() {
        std::fs::write(&full_path, &png_bytes).context("write image file")?;
    }

    // Build a smaller PNG thumbnail for the row preview.
    let thumb_h = ((THUMB_TARGET_WIDTH as f32) * (height as f32) / (width as f32))
        .round()
        .max(1.0) as u32;
    let thumb_dyn =
        image::DynamicImage::ImageRgba8(buffer).thumbnail(THUMB_TARGET_WIDTH, thumb_h);
    let mut thumb_bytes: Vec<u8> = Vec::with_capacity(8 * 1024);
    thumb_dyn
        .write_to(&mut Cursor::new(&mut thumb_bytes), image::ImageFormat::Png)
        .context("encode thumb")?;

    let size_bytes = png_bytes.len() as i64;
    let (id, is_new) = repo
        .upsert(
            "image",
            None,
            Some(&filename),
            Some(&thumb_bytes),
            Some(width as i64),
            Some(height as i64),
            size_bytes,
            hash,
        )
        .context("upsert image")?;
    let _ = app.emit("clipboard:entry-captured", CapturedEvent { id, is_new });
    Ok(())
}

/// Public API for write-back. The watcher arms the guard before mutating
/// the clipboard so the next poll does not capture our own write.
pub fn write_text(text: &str, guard: &Arc<WriteGuard>) -> Result<()> {
    let mut cb = Clipboard::new().context("arboard init")?;
    guard.arm(sha256_hex(text.as_bytes()));
    cb.set_text(text.to_string()).context("set_text")?;
    Ok(())
}

pub fn write_image_base64_png(b64: &str, guard: &Arc<WriteGuard>) -> Result<()> {
    let png = STANDARD.decode(b64.as_bytes()).context("decode base64")?;
    let dyn_img = ImageReader::new(Cursor::new(&png))
        .with_guessed_format()
        .context("guess image format")?
        .decode()
        .context("decode image")?
        .to_rgba8();
    let (w, h) = (dyn_img.width(), dyn_img.height());
    let raw = dyn_img.into_raw();
    guard.arm(sha256_hex(&raw));

    let mut cb = Clipboard::new().context("arboard init")?;
    cb.set_image(ImageData {
        width: w as usize,
        height: h as usize,
        bytes: Cow::Owned(raw),
    })
    .context("set_image")?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
