use env_logger::Target;
use log::info;
use crate::backend::{Backend, BackendType, SessionStore};
use crate::backend::tidal::Tidal;
use crate::library::Library;

mod backend;
mod library;

fn main() {
    env_logger::Builder::from_default_env()
        .target(Target::Stdout)
        .filter_level(log::LevelFilter::Info)
        .init();

    let library = Library::init("./test_library".to_string());

    let session_store = SessionStore::init(BackendType::Tidal);
    let tidal_backend = session_store.load::<Tidal>().unwrap_or_else(|| Tidal::init());
    session_store.save(&tidal_backend);

    let albums = tidal_backend.get_favorite_albums().unwrap();
    for album in albums {
        if !library.is_album_exists(&album) {
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

                let track_source = tidal_backend.download_track(&track).unwrap();
                library.save_track(&track, &track_source, &cover_source).unwrap();
            }
        }
    }
}