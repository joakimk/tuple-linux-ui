//! Gravatar loading with an in-memory texture cache and a disk cache under `$XDG_CACHE_HOME`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::gdk;

type Callback = Box<dyn Fn(&gdk::Texture)>;

#[derive(Clone)]
pub struct AvatarLoader {
    inner: Rc<RefCell<Inner>>,
    runtime: tokio::runtime::Handle,
    offline: bool,
}

struct Inner {
    textures: HashMap<String, gdk::Texture>,
    waiting: HashMap<String, Vec<Callback>>,
}

pub fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("tuple-ui")
        .join("avatars")
}

impl AvatarLoader {
    pub fn new(runtime: tokio::runtime::Handle) -> AvatarLoader {
        let offline = std::env::var_os("TUPLE_UI_OFFLINE").is_some();
        let inner = Inner { textures: HashMap::new(), waiting: HashMap::new() };
        AvatarLoader { inner: Rc::new(RefCell::new(inner)), runtime, offline }
    }

    /// Call `callback` with the avatar texture once it is available (never for blank Gravatars).
    pub fn load(&self, url: &str, callback: impl Fn(&gdk::Texture) + 'static) {
        if url.is_empty() || self.offline {
            return;
        }
        if let Some(texture) = self.inner.borrow().textures.get(url) {
            callback(texture);
            return;
        }
        let mut inner = self.inner.borrow_mut();
        if let Some(waiting) = inner.waiting.get_mut(url) {
            waiting.push(Box::new(callback));
            return;
        }
        inner.waiting.insert(url.to_string(), vec![Box::new(callback)]);
        drop(inner);

        let (sender, receiver) = async_channel::bounded::<Option<Vec<u8>>>(1);
        let fetch_url = url.to_string();
        self.runtime.spawn_blocking(move || {
            let _ = sender.send_blocking(fetch_bytes(&fetch_url));
        });
        let loader = self.clone();
        let url = url.to_string();
        glib::spawn_future_local(async move {
            let bytes = receiver.recv().await.ok().flatten();
            loader.finish(&url, bytes);
        });
    }

    fn finish(&self, url: &str, bytes: Option<Vec<u8>>) {
        let callbacks = self.inner.borrow_mut().waiting.remove(url).unwrap_or_default();
        let Some(bytes) = bytes else { return };
        let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)) else { return };
        self.inner.borrow_mut().textures.insert(url.to_string(), texture.clone());
        for callback in callbacks {
            callback(&texture);
        }
    }
}

/// Fetch (or read from the disk cache) and return PNG/JPEG bytes, or None for blank images.
fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    let dir = cache_dir();
    let key = url.bytes().fold(0xcbf29ce484222325u64, |hash, b| (hash ^ b as u64).wrapping_mul(0x100000001b3));
    let path = dir.join(format!("{key:016x}"));
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => {
            let mut response = ureq::get(url).call().ok()?;
            let bytes = response.body_mut().read_to_vec().ok()?;
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&path, &bytes);
            bytes
        }
    };
    if is_blank(&bytes) { None } else { Some(bytes) }
}

/// Gravatar's `d=blank` fallback is a fully transparent image.
pub fn is_blank(bytes: &[u8]) -> bool {
    let Ok(image) = image::load_from_memory(bytes) else { return true };
    let rgba = image.to_rgba8();
    rgba.pixels().all(|p| p[3] == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_detection() {
        let mut transparent = image::RgbaImage::new(4, 4);
        for p in transparent.pixels_mut() {
            *p = image::Rgba([0, 0, 0, 0]);
        }
        let mut buffer = std::io::Cursor::new(Vec::new());
        transparent.write_to(&mut buffer, image::ImageFormat::Png).unwrap();
        assert!(is_blank(buffer.get_ref()));

        let mut opaque = image::RgbaImage::new(4, 4);
        for p in opaque.pixels_mut() {
            *p = image::Rgba([10, 20, 30, 255]);
        }
        let mut buffer = std::io::Cursor::new(Vec::new());
        opaque.write_to(&mut buffer, image::ImageFormat::Png).unwrap();
        assert!(!is_blank(buffer.get_ref()));
        assert!(is_blank(b"not an image"));
    }
}
