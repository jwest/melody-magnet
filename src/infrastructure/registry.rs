use std::error::Error;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::backend::{Album, BackendType};
use crate::library::MappedForPathName;

type RegistryResult<T> = Result<T, Box<dyn Error>>;

pub trait FavouriteAlbums {
    fn request_favourite_album(&self, album: &Album) -> RegistryResult<()>;
    fn is_album_exists(&self, album: &Album) -> RegistryResult<bool>;
    fn mark_album_as_synchronized(&self, album: &Album) -> RegistryResult<()>;
    fn mark_album_as_processing(&self, album: &Album) -> RegistryResult<()>;
    fn get_next_to_synchronize_and_mark_as_processing(&self) -> RegistryResult<Option<Album>>;
    fn get_stats(&self) -> RegistryResult<RegistryStats>;
}

#[derive(Debug)]
pub struct RegistryStats {
    album_requested: u64,
    album_processing: u64,
    album_synchronized: u64,
    count_total: u64,
}

#[derive(strum_macros::Display)]
enum SynchronizedState {
    Requested,
    Processing,
    Synchronized,
}

pub struct SQLiteRegistry {
    connection: Connection,
}

impl SQLiteRegistry {
    pub fn init_in_memory() -> Self {
        let connection = Connection::open_in_memory().expect("failed to open SQLite database");
        Self::setup_database(&connection).expect("failed setup database");
        Self { connection }
    }

    pub fn init(path: String) -> Self {
        let connection = Connection::open(path).expect("failed to open SQLite database");
        Self::setup_database(&connection).expect("failed setup database");
        Self { connection }
    }

    fn setup_database(connection: &Connection) -> RegistryResult<()> {
        connection.execute(
            "CREATE TABLE IF NOT EXISTS album_state (
            id         INTEGER PRIMARY KEY,
            state      TEXT NOT NULL,
            path       TEXT NOT NULL,
            backend    TEXT NOT NULL,
            details    BLOB,
            cover_url  TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
            (), // empty list of parameters.
        ).expect("failed to create `album_state` table");

        Ok(())
    }

    fn count_by_status(&self, state: SynchronizedState) -> RegistryResult<u64> {
        let result = self.connection.query_row(
            "SELECT count(*) FROM album_state WHERE state=?1",
            [ state.to_string() ],
            |row| row.get(0)
        ).unwrap_or(0);

        Ok(result)
    }
}

impl FavouriteAlbums for SQLiteRegistry {
    fn request_favourite_album(&self, album: &Album) -> RegistryResult<()> {
        self.connection.execute(
            "INSERT INTO album_state (id, state, path, backend, details, cover_url, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                &album.get_id(),
                SynchronizedState::Requested.to_string(),
                PathBuf::from(album.get_artist().path_name()).join(album.path_name()).to_str().unwrap(),
                BackendType::Tidal.to_string(),
                serde_json::to_string(&album).unwrap(),
                album.get_cover_url(),
                Utc::now().to_rfc3339(),
                Utc::now().to_rfc3339()
            ),
        )?;
        Ok(())
    }

    fn is_album_exists(&self, album: &Album) -> RegistryResult<bool> {
        let state = self.connection.query_row(
            "SELECT state FROM album_state WHERE id=?1",
            [ album.get_id() ],
            |_| Ok(true),
        ).unwrap_or(false);

        Ok(state)
    }

    fn mark_album_as_synchronized(&self, album: &Album) -> RegistryResult<()> {
        self.connection.execute(
            "UPDATE album_state SET state = ?1, updated_at = ?2 WHERE id = ?3",
            (
                SynchronizedState::Synchronized.to_string(),
                Utc::now().to_rfc3339(),
                &album.get_id(),
            ),
        )?;

        Ok(())
    }

    fn mark_album_as_processing(&self, album: &Album) -> RegistryResult<()> {
        self.connection.execute(
            "UPDATE album_state SET state = ?1, updated_at = ?2 WHERE id = ?3",
            (
                SynchronizedState::Processing.to_string(),
                Utc::now().to_rfc3339(),
                &album.get_id(),
            ),
        )?;

        Ok(())
    }

    fn get_next_to_synchronize_and_mark_as_processing(&self) -> RegistryResult<Option<Album>> {
        let mut stmt = self.connection.prepare("SELECT details FROM album_state WHERE state = 'Requested' LIMIT 1")?;
        let result = stmt.query_row([], |row| {
            let details: String = row.get(0).unwrap();
            let album: Album = serde_json::from_str(details.as_str()).unwrap();
            Ok(album)
        }).map(|album| Some(album)).unwrap_or(None);

        if result.is_some() {
            self.mark_album_as_processing(&result.clone().unwrap()).expect("Error on mark album as processing");
        }
        Ok(result)
    }

    fn get_stats(&self) -> RegistryResult<RegistryStats> {
        let album_requested = self.count_by_status(SynchronizedState::Requested)?;
        let album_processing = self.count_by_status(SynchronizedState::Processing)?;
        let album_synchronized = self.count_by_status(SynchronizedState::Synchronized)?;

        let stats = RegistryStats {
            album_requested,
            album_processing,
            album_synchronized,
            count_total: album_requested + album_processing + album_synchronized,
        };

        Ok(stats)
    }
}