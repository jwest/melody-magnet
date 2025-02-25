use std::{fs, io};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use bytes::Bytes;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::library::MappedForPathName;

pub mod tidal;

pub type AlbumId = String;
pub type ArtistId = String;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("data store disconnected")]
    Disconnect(#[from] io::Error),
    #[error("request to api failed")]
    RequestError,
}

pub type BackendResult<T> = Result<T, BackendError>;

#[derive(Debug)]
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Track {
    id: ArtistId,
    title: String,
    album: Album,
    track_number: u32,
    volume_number: u32,
    // codec ACC
    // Track { id: "83516195", title: "Whatever Lola Wants", album: Album { id: "83516182", artist: Artist { id: "3968881", name: "Bob & Ray" }, title: "Bob And Ray Throw A Stereo Spectacular", release_date: 1900-01-07, number_of_volumes: 1, cover_url: Some("https://resources.tidal.com/images/7b0e1c3d/0718/4669/a7a5/1735f7659610/640x640.jpg"), number_of_tracks: 15 }, track_number: 13, volume_number: 1 }
}

impl Track {
    pub fn get_album(&self) -> Album {
        self.album.clone()
    }
    pub fn get_volume_number(&self) -> u32 {
        self.volume_number
    }
    pub fn get_title(&self) -> String {
        self.title.clone()
    }
    pub fn get_track_number(&self) -> u32 {
        self.track_number
    }
}

impl MappedForPathName for Track {
    fn path_name(&self) -> String {
        format!("{:02} {} - {}.flac", self.track_number, sanitize_name(self.title.as_str()), sanitize_name(self.album.artist.name.as_str()))
    }
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Artist {
    id: ArtistId,
    name: String,
}

impl Artist {
    pub fn get_name(&self) -> String {
        self.name.clone()
    }
}

impl MappedForPathName for Artist {
    fn path_name(&self) -> String {
        sanitize_name(self.name.as_str())
    }
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Album {
    id: AlbumId,
    artist: Artist,
    title: String,
    // #[serde(with = "ts_seconds_option")]
    release_date: NaiveDate,
    number_of_volumes: u32,
    cover_url: Option<String>,
    number_of_tracks: u32,
}

impl Album {
    pub fn get_id(&self) -> AlbumId {
        self.id.clone()
    }
    pub fn get_artist(&self) -> Artist {
        self.artist.clone()
    }
    pub fn is_few_volumes(&self) -> bool {
        self.number_of_volumes > 1
    }
    pub fn get_title(&self) -> String {
        self.title.clone()
    }
    pub fn get_number_of_tracks(&self) -> u32 {
        self.number_of_tracks
    }
    pub fn get_cover_url(&self) -> Option<String> {
        self.cover_url.clone()
    }
}

impl MappedForPathName for Album {
    fn path_name(&self) -> String {
        format!("{} {}", self.release_date.year(), sanitize_name(self.title.as_str()))
    }
}

pub struct Pagination {
    limit: u64,
    offset: u64,
}

impl Pagination {
    pub fn init(limit: u64) -> Self {
        Self { limit, offset: 0 }
    }
    pub fn get_limit(&self) -> u64 {
        self.limit
    }
    pub fn get_offset(&self) -> u64 {
        self.offset
    }
}

pub trait Backend {
    fn get_favorite_albums(&self, pagination: Pagination) -> BackendResult<Vec<Album>>;

    fn get_album_tracks(&self, album: &Album) -> BackendResult<Vec<Track>>;

    fn download_track(&self, track: &Track) -> BackendResult<Bytes>;

    fn download_album_cover(&self, album_id: &Album) -> BackendResult<Bytes>;

    fn serialize(&self) -> String;
    fn deserialize(serialized: String) -> Self where Self: Sized;
}

#[derive(strum_macros::Display)]
pub enum BackendType {
    Tidal,
}

pub struct SessionStore {
    path: String,
    backend_type: BackendType,
}

impl SessionStore {
    pub fn init(path: String, backend_type: BackendType) -> Self {
        Self { path, backend_type }
    }

    pub fn load<T: Backend + 'static>(&self) -> Option<T> {
        if !self.file_path().exists() {
            return None;
        }
        match fs::read_to_string(self.file_path()) {
            Ok(serialized) => Some(T::deserialize(serialized)),
            Err(_) => None
        }
    }

    pub fn save<T: Backend + 'static>(&self, backend: &T) {
        if self.file_path().exists() {
            let _ = fs::remove_file(self.file_path());
        }
        let serialized = backend.serialize();
        let mut file = File::create(self.file_path()).unwrap();
        file.write_all(serialized.as_bytes()).unwrap();
    }

    fn file_path(&self) -> PathBuf {
        let store_path = PathBuf::from(self.path.clone());
        match self.backend_type {
            BackendType::Tidal => store_path.join("tidal_session.json".to_string()).to_owned().to_path_buf(),
        }
    }
}

fn sanitize_name(input: &str) -> String {
    input
        .replace("/","_")
        .replace("\\","_")
}