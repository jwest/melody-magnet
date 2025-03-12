use bytes::Bytes;
use chrono::NaiveDate;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::backend::{Album, Artist, Backend, BackendError, BackendResult, Pagination, Track};
use crate::backend::tidal::session::TidalSession;

pub mod session;

const FAVOURITE_ITEMS_PER_PAGE: usize = 100;

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Tidal {
    session: TidalSession,
}

impl Tidal {
    pub fn init() -> Self {
        Self {
            session: TidalSession::setup(),
        }
    }

    pub(crate) fn refresh_token(&mut self) -> BackendResult<()> {
        self.session.refresh_token().unwrap();
        Ok(())
    }
}

impl Backend for Tidal {
    fn get_favorite_albums(&self) -> BackendResult<Vec<Album>> {
        let mut albums: Vec<Album> = vec![];
        let pagination = Pagination::init(FAVOURITE_ITEMS_PER_PAGE);

        for page in pagination {
            debug!("Tidal::get_favorite_albums: page={:?}", &page);

            let v = self.session.get_favorite_albums(page.limit, page.offset);

            if v.is_err() {
                error!("Backend error: {:?}", v.err().unwrap());
                return Err(BackendError::RequestError);
            }

            if let Value::Array(items) = &v.unwrap()["items"] {
                if items.is_empty() {
                    break;
                }

                for item in items {
                    // Parse item based on tidal API output
                    // {
                    //      "created": String("2020-08-10T12:52:04.605+0000"),
                    //      "item": Object {
                    //          "adSupportedStreamReady": Bool(true),
                    //          "allowStreaming": Bool(true),
                    //          "artist": Object {
                    //              "handle": Null,
                    //              "id": Number(3566483),
                    //              "name": String("Max Richter"),
                    //              "picture": String("83eeab6d-8a8a-4154-ba9d-160db15afcf2"),
                    //              "type": String("MAIN")
                    //          },
                    //          "artists": Array [
                    //              Object {
                    //                  "handle": Null,
                    //                  "id": Number(3566483),
                    //                  "name": String("Max Richter"),
                    //                  "picture": String("83eeab6d-8a8a-4154-ba9d-160db15afcf2"),
                    //                  "type": String("MAIN")
                    //              }
                    //          ],
                    //          "audioModes": Array [String("STEREO")],
                    //          "audioQuality": String("LOSSLESS"),
                    //          "copyright": String("© 2014 Deutsche Grammophon GmbH, Berlin"),
                    //          "cover": String("8cd770fb-f51b-4b9f-a120-31d274bd8ffe"),
                    //          "djReady": Bool(true),
                    //          "duration": Number(2296),
                    //          "explicit": Bool(false),
                    //          "id": Number(28477014),
                    //          "mediaMetadata": Object {"tags": Array [String("LOSSLESS")]},
                    //          "numberOfTracks": Number(27),
                    //          "numberOfVideos": Number(0),
                    //          "numberOfVolumes": Number(1),
                    //          "popularity": Number(43),
                    //          "premiumStreamingOnly": Bool(false),
                    //          "releaseDate": String("2014-01-01"),
                    //          "stemReady": Bool(false),
                    //          "streamReady": Bool(true),
                    //          "streamStartDate": String("2021-03-24T00:00:00.000+0000"),
                    //          "title": String("24 Postcards In Full Colour"),
                    //          "type": String("ALBUM"),
                    //          "upc": String("00028947933151"),
                    //          "upload": Bool(false),
                    //          "url": String("http://www.tidal.com/album/28477014"),
                    //          "version": Null,
                    //          "vibrantColor": String("#ede62f"),
                    //          "videoCover": Null
                    //      }
                    // }

                    if item["item"]["adSupportedStreamReady"].as_bool().is_some_and(|ready| ready) {
                        let album_id = item["item"]["id"].as_i64().unwrap().to_string();

                        let cover_url = if let Some(cover_id) = item["item"]["cover"].as_str() {
                            let cover_size = CoverSize::CoverSize640 as usize;
                            Some(format!("https://resources.tidal.com/images/{}/{}x{}.jpg", cover_id.replace('-', "/"), cover_size, cover_size))
                        } else {
                            None
                        };

                        albums.push(Album {
                            id: album_id.clone(),
                            artist: Artist {
                                id: item["item"]["artist"]["id"].as_i64().unwrap().to_string(),
                                name: item["item"]["artist"]["name"].as_str().unwrap().to_string(),
                            },
                            title: item["item"]["title"].as_str().unwrap().to_string(),
                            release_date: NaiveDate::parse_from_str(item["item"]["releaseDate"].as_str().unwrap(), "%Y-%m-%d").unwrap(),
                            number_of_volumes: item["item"]["numberOfVolumes"].as_i64().unwrap() as u32,
                            number_of_tracks: item["item"]["numberOfTracks"].as_i64().unwrap() as u32,
                            cover_url,
                        })
                    }
                }
            }
        }

        Ok(albums)
    }

    fn get_album_tracks(&self, album: &Album) -> BackendResult<Vec<Track>> {
        let album_details = self.session.get_album(album.id.as_str()).unwrap();

        let mut tracks: Vec<Track> = vec![];

        if let Value::Array(items) = &album_details["items"] {
            for item in items {
                // parse track based on Tidal API output
                // {
                //  "adSupportedStreamReady": Bool(true),
                //  "album": Object {
                //      "cover": String("7f173d78-8a83-4c78-9e34-68e08bf15e07"),
                //      "id": Number(4911986),
                //      "title": String("111 Years of Deutsche Grammophon"),
                //      "vibrantColor": String("#c6c166"),
                //      "videoCover": Null
                //  },
                //  "allowStreaming": Bool(true),
                //  "artist": Object {
                //      "handle": Null,
                //      "id": Number(577),
                //      "name": String("Cecilia Bartoli"),
                //      "picture": String("974fff3c-0887-4710-8e66-09a6ffb2facb"),
                //      "type": String("MAIN")
                //  },
                //  "artists": Array [
                //      Object {
                //          "handle": Null,
                //          "id": Number(577),
                //          "name": String("Cecilia Bartoli"),
                //          "picture": String("974fff3c-0887-4710-8e66-09a6ffb2facb"),
                //          "type": String("MAIN")
                //      },
                //      Object {
                //          "handle": Null,
                //          "id": Number(3652537),
                //          "name": String("Wiener Philharmoniker"),
                //          "picture": String("eed38926-f05f-406b-8f94-00b59c3a8f77"),
                //          "type": String("FEATURED")
                //      },
                //      Object {
                //          "handle": Null,
                //          "id": Number(8429),
                //          "name": String("Claudio Abbado"),
                //          "picture": String("5223ce49-821d-4fc9-9617-867574d21d71"),
                //          "type": String("FEATURED")
                //      }
                //  ],
                //  "audioModes": Array [String("STEREO")],
                //  "audioQuality": String("LOSSLESS"),
                //  "bpm": Null,
                //  "copyright": String("℗ 1994 Deutsche Grammophon GmbH, Berlin"),
                //  "djReady": Bool(true),
                //  "duration": Number(147),
                //  "editable": Bool(false),
                //  "explicit": Bool(false),
                //  "id": Number(4911993),
                //  "isrc": String("DEF059430527"),
                //  "mediaMetadata": Object {
                //      "tags": Array [String("LOSSLESS")]
                //  },
                //  "mixes": Object {
                //      "TRACK_MIX": String("001680c45a4baf428ec3b9f5453b66")
                //  },
                //  "peak": Number(0.519287),
                //  "popularity": Number(16),
                //  "premiumStreamingOnly": Bool(false),
                //  "replayGain": Number(-7.38),
                //  "stemReady": Bool(false),
                //  "streamReady": Bool(true),
                //  "streamStartDate": String("2010-08-31T00:00:00.000+0000"),
                //  "title": String("Mozart: Le nozze di Figaro, K. 492, Act II: No. 12, Voi che sapete"),
                //  "trackNumber": Number(7),
                //  "upload": Bool(false),
                //  "url": String("http://www.tidal.com/track/4911993"),
                //  "version": Null,
                //  "volumeNumber": Number(1)
                // }

                if item["adSupportedStreamReady"].as_bool().is_some_and(|ready| ready) {
                    let track = Track {
                        id: item["id"].as_i64().unwrap().to_string(),
                        title: item["title"].as_str().unwrap_or_default().to_string(),
                        track_number: item["trackNumber"].as_u64().unwrap() as u32,
                        volume_number: item["volumeNumber"].as_u64().unwrap() as u32,
                        album: album.clone(),
                    };

                    tracks.push(track);
                }
            }
        }

        Ok(tracks)
    }

    fn download_track(&self, track: &Track) -> BackendResult<Bytes> {
        for _ in 1..5 {
            match self.session.get_track_bytes(track.id.clone()) {
                Ok(bytes) => return Ok(bytes),
                Err(err) => info!("error downloading track, retry... ({:?})", err),
            }
        }

        Err(BackendError::RequestError)
    }

    fn download_album_cover(&self, album: &Album) -> BackendResult<Bytes> {
        for _ in 1..5 {
            match self.session.get_cover_bytes(album.cover_url.clone().unwrap().clone()) {
                Ok(bytes) => return Ok(bytes),
                Err(err) => info!("error downloading track, retry... ({:?})", err),
            }
        }

        Err(BackendError::RequestError)
    }

    fn serialize(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }

    fn deserialize(serialized: String) -> Self {
        serde_json::from_str(&serialized).unwrap()
    }
}

enum CoverSize {
    CoverSize80 = 80,
    CoverSize160 = 160,
    CoverSize320 = 320,
    CoverSize640 = 640,
    CoverSize1280 = 1280,
}