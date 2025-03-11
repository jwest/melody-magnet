use bytes::Bytes;
use reqwest::blocking::{Client, Response};
use reqwest::{header, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::error::Error;
use std::time::Duration;
use std::thread;
use log::{error, info};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TidalClientError {
    #[error("Error on getting track url")]
    GettingTrackUrlError,
    #[error("The token has expired")]
    AuthorizationError,
    #[error("Request error")]
    RequestError,
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct TidalSession {
    token_type: String,
    access_token: String,
    refresh_token: String,

    session_id: String,
    country_code: String,
    user_id: i64,
    token: String,
    api_path: String,
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseTidalSession {
    session_id: String,
    country_code: String,
    user_id: i64,
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseMedia {
    urls: Vec<String>,
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceAuthorization {
    verification_uri_complete: String,
    device_code: String,
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
struct RefreshAuthorization {
    token_type: String,
    access_token: String,
}

const CLIENT_ID: &'static str = "zU4XHVVkc2tDPo4t";
const CLIENT_SECRET: &'static str = "VJKhDFqJPqvsPVNBV6ukXTJmwlvbttP7wlMlrc72se4%3D";

impl DeviceAuthorization {
    fn format_url(&self) -> String {
        format!("https://{}", self.verification_uri_complete)
    }
    fn wait_for_link(&self) -> Result<ResponseSession, Box<dyn Error>> {
        let client = Client::builder().build()?;

        for _ in 0..60 {
            thread::sleep(Duration::from_secs(2));

            let params = &[
                ("client_id", CLIENT_ID),
                ("client_secret", CLIENT_SECRET),
                ("device_code", &self.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("scope", "r_usr w_usr w_sub"),
            ];

            let res = client.post("https://auth.tidal.com:443/v1/oauth2/token")
                .form(params)
                .send()?;

            info!("[Session] token resposne: {:?}", res.status());

            if res.status().is_success() {
                let session_response = res.json::<ResponseSession>()?;
                return Ok(session_response);
            }
        }

        self.wait_for_link()
    }
}

#[derive(Debug)]
#[derive(Deserialize)]
struct ResponseSession {
    access_token: String,
    refresh_token: String,
    token_type: String,
}

impl ResponseSession {
    pub fn token(&self) -> String {
        format!("{} {}", self.token_type, self.access_token)
    }
}

impl TidalSession {
    pub fn setup() -> TidalSession {
        let device_auth = TidalSession::login_link().unwrap();

        let session_response = device_auth.wait_for_link().unwrap();

        TidalSession::init(session_response).unwrap()
    }
    pub fn refresh_token(&mut self) -> Result<(), Box<dyn Error>> {
        let refreshed_session = Self::refresh_access_token(self.refresh_token.clone())?;

        self.token_type = refreshed_session.token_type.clone();
        self.token = refreshed_session.access_token.clone();
        self.refresh_token = refreshed_session.refresh_token.clone();

        Ok(())
    }

    fn login_link() -> Result<DeviceAuthorization, Box<dyn Error>> {
        let client = Client::builder()
            .build()?;
        let res = client.post("https://auth.tidal.com:443/v1/oauth2/device_authorization")
            .form(&[("client_id", CLIENT_ID), ("scope", "r_usr+w_usr+w_sub")])
            .send()?;

        let device_auth_response = res.json::<DeviceAuthorization>()?;
        info!("[Session] login link: {}, waiting...", device_auth_response.format_url());

        Ok(device_auth_response)
    }
    fn init(config: ResponseSession) -> Result<TidalSession, Box<dyn Error>> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, header::HeaderValue::from_str(config.token().as_str()).unwrap());

        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()?;
        let res = client.get("https://api.tidal.com/v1/sessions").send()?;

        if res.status().is_success() {
            let session = res.json::<ResponseTidalSession>()?;

            info!("[Session] {:?}", session);

            return Ok(TidalSession {
                session_id: session.session_id,
                country_code: session.country_code,
                user_id: session.user_id,
                token: config.token().clone(),
                api_path: "https://api.tidal.com/v1".to_string(),
                access_token: config.access_token.clone(),
                refresh_token: config.refresh_token.clone(),
                token_type: config.token_type.clone(),
            });
        }

        info!("[Session] outdated, refresh needed, {:?}", res);

        Self::init(Self::refresh_access_token(config.refresh_token)?)
    }
    fn refresh_access_token(refresh_token: String) -> Result<ResponseSession, Box<dyn Error>> {
        let client = Client::builder()
            .build()?;
        let res = client.post("https://auth.tidal.com:443/v1/oauth2/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
                ("client_id", CLIENT_ID),
                ("client_secret", CLIENT_SECRET)
            ])
            .send()?;

        let refresh_auth_response = res.json::<RefreshAuthorization>()?;
        info!("[Session] refreshed with success");

        Ok(ResponseSession {
            token_type: refresh_auth_response.token_type,
            access_token: refresh_auth_response.access_token,
            refresh_token,
        })
    }
    fn build_client(&self) -> Client {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, header::HeaderValue::from_str(self.token.as_str()).unwrap());

        Client::builder()
            .default_headers(headers)
            .build().unwrap()
    }
    fn request(&self, url: String) -> Result<Response, Box<dyn Error>> {
        let res = self.build_client().get(url)
            .header(header::AUTHORIZATION, header::HeaderValue::from_str(format!("Bearer {}", self.token).as_str()).unwrap())
            .send()?;
        Ok(res)
    }
    pub(super) fn get_favorite_albums(&self, limit: usize, offset: usize) -> Result<Value, Box<dyn Error>> {
        let response = self.request(format!("{}/users/{}/favorites/albums?sessionId={}&countryCode={}&limit={}&offset={}", self.api_path, self.user_id, self.session_id, self.country_code, limit, offset))?;

        match response.status() {
            StatusCode::OK => {
                let body = response.text()?;
                let result: Value = serde_json::from_str(&body)?;
                Ok(result)
            },
            StatusCode::UNAUTHORIZED => {
                error!("Tidal client request error, {:?}", response.text());
                Err(TidalClientError::AuthorizationError.into())
            },
            _ => {
                error!("Tidal client request error, {:?}, {:?}", response.status(), response.text());
                Err(TidalClientError::RequestError.into())
            }
        }
    }
    pub(super) fn get_album(&self, album_id: &str) -> Result<Value, Box<dyn Error>> {
        let response = self.request(format!("{}/albums/{}/tracks?countryCode={}&deviceType=BROWSER", self.api_path, album_id, self.country_code))?;
        let body = response.text()?;
        let result: Value = serde_json::from_str(&body)?;
        Ok(result)
    }
    fn get_track_url(&self, track_id: String) -> Result<String, Box<dyn Error>> {
        let mut url: Option<String> = None;
        for quality in vec!["HI_RES_LOSSLESS", "LOSSLESS", "HIGH"] {
            let download_url = format!("{}/tracks/{}/urlpostpaywall?sessionId={}&urlusagemode=STREAM&audioquality={}&assetpresentation=FULL", self.api_path, track_id, self.session_id, quality);
            info!("Download track: {}, with url: {}", track_id, download_url);
            let response = self.request(download_url)?;
            if response.status().is_success() {
                url = Some(response.json::<ResponseMedia>()?.urls[0].clone());
                break;
            }
        }

        match &url {
            Some(url) => Ok(url.clone()),
            None => Err(TidalClientError::GettingTrackUrlError.into()),
        }
    }
    pub(super) fn get_track_bytes(&self, track_id: String) -> Result<Bytes, Box<dyn Error>> {
        let url = self.get_track_url(track_id.clone())?;
        let file_response = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?.get(url).send()?;

        Ok(file_response.bytes()?)
    }
    pub(super) fn get_cover_bytes(&self, cover_url: String) -> Result<Bytes, Box<dyn Error>> {
        let file_response = Client::builder()
            .timeout(Duration::from_secs(500))
            .build()?
            .get(&cover_url).send()?
            .bytes()?;

        Ok(file_response)
    }
}
