use dotenvy::dotenv;
use env_logger::Target;
use log::{error, info, warn};
use std::thread;
use std::time::Duration;
use crate::backend::{Backend, BackendType, SessionStore};
use crate::backend::tidal::Tidal;
use crate::infrastructure::config::Config;
use library::registry::{FavouriteAlbums, SQLiteRegistry};
use crate::library::Library;

mod backend;
mod library;
mod infrastructure;
mod http;

fn main() {
    dotenv().ok();
    let config = Config::init().expect("Config initialization error!");

    env_logger::Builder::from_default_env()
        .target(Target::Stdout)
        .filter_level(log::LevelFilter::Info)
        .init();

    let interval_secs = config.sync_interval_seconds.unwrap_or(300);
    info!("Starting background worker with interval: {}s", interval_secs);

    let worker_config = config.clone();
    thread::spawn(move || {
        loop {
            sync_favourites();
            thread::sleep(Duration::from_secs(interval_secs));
        }
    });

    http::start_http_server(worker_config);
}

fn sync_favourites() {
    info!("Sync favourites cycle started");

    let config = Config::init().expect("Config initialization error!");
    let registry = SQLiteRegistry::init(config.database_file_path);
    let library = Library::init(config.library_path);

    let session_store = SessionStore::init(config.session_store_path, BackendType::Tidal);
    let mut tidal_backend = session_store.load::<Tidal>().unwrap_or_else(|| Tidal::init());

    while let Some(album) = registry.get_next_to_synchronize_and_mark_as_processing().expect("problem with database") {
        print_stats(&registry);

        if !library.is_album_exists(&album) {
            registry.mark_album_as_processing(&album).unwrap();

            // start progress tracking
            let album_id = album.get_id();
            let artist_name = album.get_artist().get_name();
            let title = album.get_title();
            let total_tracks = album.get_number_of_tracks();
            http::progress::start_album(album_id.clone(), artist_name.clone(), title.clone(), total_tracks);

            // if canceled before starting, stop early
            if http::progress::is_canceled(album_id.as_str()) {
                http::progress::mark_canceled(album_id.as_str());
                http::progress::clear(album_id.as_str());
                let _ = registry.delete_album_by_id(album_id.as_str());
                continue;
            }

            let tracks = tidal_backend.get_album_tracks(&album).unwrap();

            let cover_source = if album.get_cover_url().is_some() {
                let cover = tidal_backend.download_album_cover(&album).unwrap();
                library.save_album_cover(&album, &cover).unwrap();
                Some(cover)
            } else {
                None
            };

            for track in tracks {
                if http::progress::is_canceled(album_id.as_str()) {
                    http::progress::mark_canceled(album_id.as_str());
                    http::progress::clear_cancel(album_id.as_str());
                    let _ = registry.delete_album_by_id(album_id.as_str());
                    break;
                }
                info!("track: {:?}", track);

                let _ = tidal_backend.download_track(&track).and_then(|track_source| {
                    if library.save_track(&track, &track_source, &cover_source).is_err() {
                        error!("Failed to save track");
                    }
                    let bytes = track_source.len() as u64;
                    http::progress::track_saved(album_id.as_str(), bytes);
                    Ok(())
                });
            }

            if !http::progress::is_canceled(album_id.as_str()) {
                http::progress::album_done(album_id.as_str());
                http::progress::clear(album_id.as_str());
                let _ = registry.mark_album_as_synchronized(&album);
            } else {
                http::progress::clear(album_id.as_str());
            }
        } else {
            library.remove_album(&album);
        }
    }

    print_stats(&registry);

    match tidal_backend.get_favorite_albums() {
        Ok(favourite_albums) => {
            for album in favourite_albums {
                if !&registry.is_album_exists(&album).expect("problem with database") {
                    let _ = &registry.request_favourite_album(&album).unwrap();
                    info!("Album requested to synchronize: {:?}", &album);
                }
            }

            print_stats(&registry);
        },
        Err(err) => {
            warn!("Probably token expire, refreshing... ({:?})", err);
            tidal_backend.refresh_token().unwrap();
            session_store.save(&tidal_backend);
        }
    }
}

fn print_stats(registry: &SQLiteRegistry) {
    let stats = registry.get_stats().expect("problem with aggregate statistics");
    info!("Current sync stats: {:?}", stats);
}