use std::error::Error;
use std::fs;
use std::path::PathBuf;
use bytes::Bytes;
use metaflac::block::PictureType;
use metaflac::Tag;
use crate::backend::{Album, Track};

pub mod registry;

pub struct Library {
    path: PathBuf,
}

impl Library {
    pub fn init(path: String) -> Self {
        Self { path: PathBuf::from(path) }
    }
    pub fn is_album_exists(&self, album: &Album) -> bool {
        self.path.clone().join(album.get_artist().path_name()).join(album.path_name()).exists()
    }

    pub fn save_track(&self, track: &Track, source: &Bytes, cover_source: &Option<Bytes>) -> Result<(), Box<dyn Error>> {
        let volume_path = self.get_volume_path(track);
        fs::create_dir_all(&volume_path)?;

        let track_path = self.get_track_path(track);
        fs::write(&track_path, source)?;

        self.save_track_meta(&track, cover_source)?;
        Ok(())
    }

    pub fn save_album_cover(&self, album: &Album, source: &Bytes) -> Result<(), Box<dyn Error>> {
        let album_path = self.get_album_path(album);

        fs::create_dir_all(&album_path)?;
        let cover_path = album_path.join("cover.png");

        fs::write(&cover_path, source)?;

        Ok(())
    }

    fn get_album_path(&self, album: &Album) -> PathBuf {
        self.path.clone().join(album.get_artist().path_name()).join(album.path_name())
    }

    fn get_volume_path(&self, track: &Track) -> PathBuf {
        let album_path = self.get_album_path(&track.get_album());

        if track.get_album().is_few_volumes() {
            album_path.join(format!("CD{:02}", track.get_volume_number()))
        } else {
            album_path
        }
    }

    fn get_track_path(&self, track: &Track) -> PathBuf {
        self.get_volume_path(track).join(track.path_name())
    }

    fn save_track_meta(&self, track: &Track, cover_source: &Option<Bytes>) -> Result<(), Box<dyn Error>> {
        let mut tag = Tag::read_from_path(self.get_track_path(track))?;
        let vorbis = tag.vorbis_comments_mut();
        vorbis.set_track(track.get_track_number());
        vorbis.set_total_tracks(track.get_album().get_number_of_tracks());
        vorbis.set_title(vec![track.get_title()]);
        vorbis.set_album(vec![track.get_album().get_title()]);
        vorbis.set_artist(vec![track.get_album().get_artist().get_name()]);

        if let Some(cover) = cover_source {
            tag.add_picture("image/png", PictureType::CoverFront, cover.to_vec());
        }

        tag.save()?;
        Ok(())
    }
}

pub trait MappedForPathName {
    fn path_name(&self) -> String;
}