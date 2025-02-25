use dotenvy::dotenv;
use env_logger::Target;
use log::{error, info};
use crate::backend::{Backend, BackendType, Pagination, SessionStore};
use crate::backend::tidal::Tidal;
use crate::infrastructure::config::Config;
use crate::infrastructure::registry::{FavouriteAlbums, SQLiteRegistry};
use crate::library::Library;

mod backend;
mod library;
mod infrastructure;

fn main() {
    dotenv().ok();

    env_logger::Builder::from_default_env()
        .target(Target::Stdout)
        .filter_level(log::LevelFilter::Info)
        .init();

    let config = Config::init().expect("Config initialization error!");
    let registry = SQLiteRegistry::init(config.database_file_path);
    let library = Library::init(config.library_path);

    let session_store = SessionStore::init(config.session_store_path, BackendType::Tidal);
    let tidal_backend = session_store.load::<Tidal>().unwrap_or_else(|| Tidal::init());
    session_store.save(&tidal_backend);

    let favourite_albums = tidal_backend.get_favorite_albums(Pagination::init(5)).unwrap();
    for album in favourite_albums {
        if !registry.is_album_exists(&album).expect("problem with database") {
            registry.request_favourite_album(&album).unwrap();
        }
    }

    while let Some(album) = registry.get_next_to_synchronize_and_mark_as_processing().expect("problem with database") {
        let stats = registry.get_stats().expect("problem with aggregate statistics");
        info!("Current stats: {:?}", stats);

        if !library.is_album_exists(&album) {
            registry.mark_album_as_processing(&album).unwrap();

            let tracks = tidal_backend.get_album_tracks(&album).unwrap();

            let cover_source = if album.get_cover_url().is_some() {
                let cover = tidal_backend.download_album_cover(&album).unwrap();
                library.save_album_cover(&album, &cover).unwrap();
                Some(cover)
            } else {
                None
            };

            for track in tracks {
                info!("track: {:?}", track);

                let _ = tidal_backend.download_track(&track).and_then(|track_source| {
                    if library.save_track(&track, &track_source, &cover_source).is_err() {
                        error!("Failed to save track");
                    }
                    registry.mark_album_as_synchronized(&album).unwrap();
                    Ok(())
                });
            }
        }
    }
}