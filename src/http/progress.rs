use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct AlbumProgress {
    pub album_id: String,
    pub artist: String,
    pub title: String,
    pub total_tracks: u32,
    pub tracks_downloaded: u32,
    pub bytes_downloaded: u64,
    pub state: String,
}

static PROGRESS: Lazy<Mutex<HashMap<String, AlbumProgress>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static CANCELED: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

pub fn start_album(album_id: String, artist: String, title: String, total_tracks: u32) {
    let mut map = PROGRESS.lock().unwrap();
    map.insert(album_id.clone(), AlbumProgress {
        album_id,
        artist,
        title,
        total_tracks,
        tracks_downloaded: 0,
        bytes_downloaded: 0,
        state: "Processing".to_string(),
    });
}

pub fn track_saved(album_id: &str, bytes: u64) {
    if let Some(p) = PROGRESS.lock().unwrap().get_mut(album_id) {
        p.tracks_downloaded = p.tracks_downloaded.saturating_add(1);
        p.bytes_downloaded = p.bytes_downloaded.saturating_add(bytes);
    }
}

pub fn album_done(album_id: &str) {
    if let Some(p) = PROGRESS.lock().unwrap().get_mut(album_id) {
        p.state = "Synchronized".to_string();
    }
}

pub fn mark_canceled(album_id: &str) {
    if let Some(p) = PROGRESS.lock().unwrap().get_mut(album_id) {
        p.state = "Canceled".to_string();
    }
}

pub fn request_cancel(album_id: &str) {
    let mut set = CANCELED.lock().unwrap();
    set.insert(album_id.to_string());
}

pub fn is_canceled(album_id: &str) -> bool {
    CANCELED.lock().unwrap().contains(album_id)
}

pub fn clear_cancel(album_id: &str) {
    let mut set = CANCELED.lock().unwrap();
    set.remove(album_id);
}

pub fn clear(album_id: &str) { let _ = PROGRESS.lock().unwrap().remove(album_id); }

pub fn snapshot() -> HashMap<String, AlbumProgress> { PROGRESS.lock().unwrap().clone() }
