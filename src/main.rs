use cronjob::CronJob;
use dotenvy::dotenv;
use env_logger::Target;
use log::{error, info, warn};
use crate::backend::{Backend, BackendType, SessionStore};
use crate::backend::tidal::Tidal;
use crate::infrastructure::config::Config;
use library::registry::{FavouriteAlbums, SQLiteRegistry};
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

    let mut cron_sync = CronJob::new("Sync favourites", sync_favourites);
    cron_sync.seconds("0,5,10,15,20,25,30,35,40,45,50,55");
    cron_sync.start_job();
}

fn sync_favourites(_: &str) {
    info!("Sync favourites cron job started");

    let config = Config::init().expect("Config initialization error!");
    let registry = SQLiteRegistry::init(config.database_file_path);
    let library = Library::init(config.library_path);

    let session_store = SessionStore::init(config.session_store_path, BackendType::Tidal);
    let mut tidal_backend = session_store.load::<Tidal>().unwrap_or_else(|| Tidal::init());

    while let Some(album) = registry.get_next_to_synchronize_and_mark_as_processing().expect("problem with database") {
        print_stats(&registry);

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