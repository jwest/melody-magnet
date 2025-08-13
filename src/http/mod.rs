use std::collections::HashMap;

use log::{error, info};
use serde::Serialize;
use tiny_http::{Header, Method, Response, Server, StatusCode};
use crate::http::ui::AlbumRow;
use crate::infrastructure::config::Config;
use crate::library::registry::{SQLiteRegistry, FavouriteAlbums};

pub mod progress;
pub mod ui;

pub fn start_http_server(config: Config) {
    let http_enabled = config.http_enabled.unwrap_or(true);
    if !http_enabled {
        info!("HTTP server disabled by config");
        return;
    }

    let host = config.http_host.clone().unwrap_or_else(|| "0.0.0.0".to_string());
    let port = config.http_port.unwrap_or(8080);
    let addr = format!("{}:{}", host, port);

    let server = Server::http(&addr).expect("Failed to start HTTP server");
    info!("HTTP server listening on http://{}", addr);

    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().clone();
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);

        match (method, url.as_str()) {
            (Method::Get, "/") => {
                let response = Response::from_string(ui::index_html())
                    .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap());
                let _ = request.respond(response);
            }
            (Method::Get, "/api/health") => {
                let now = chrono::Utc::now().to_rfc3339();
                let payload = serde_json::json!({
                    "status": "ok",
                    "version": env!("CARGO_PKG_VERSION"),
                    "time": now
                });
                let _ = request.respond(json_response(payload.to_string()));
            }
            (Method::Get, "/api/stats") => {
                let registry = SQLiteRegistry::init(config.database_file_path.clone());
                match registry.get_stats() {
                    Ok(stats) => {
                        let dto = StatsDto::from(stats);
                        let _ = request.respond(json_response(serde_json::to_string(&dto).unwrap()));
                    }
                    Err(e) => {
                        error!("stats error: {:?}", e);
                        let _ = request.respond(error_response(StatusCode(500), "stats error"));
                    }
                }
            }
            (Method::Get, path) if path.starts_with("/api/albums") => {
                let query = url.splitn(2, '?').nth(1).unwrap_or("");
                let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
                let state = params.get("state").map(|s| s.to_string()).unwrap_or("Synchronized".to_string());
                let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(10000);
                let offset: i64 = params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);

                match list_albums(&config.database_file_path, &state, limit, offset) {
                    Ok(albums) => {
                        let _ = request.respond(json_response(serde_json::to_string(&albums).unwrap()));
                    }
                    Err(e) => {
                        error!("albums error: {:?}", e);
                        let _ = request.respond(error_response(StatusCode(500), "albums error"));
                    }
                }
            }
            (Method::Get, "/api/progress") => {
                let snapshot = progress::snapshot();
                let _ = request.respond(json_response(serde_json::to_string(&snapshot).unwrap()));
            }
            (Method::Post, path) if path.starts_with("/api/cancel") => {
                let mut album_id: Option<String> = None;
                let query = url.splitn(2, '?').nth(1).unwrap_or("");
                if !query.is_empty() {
                    let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
                    album_id = params.get("id").cloned();
                }
                if album_id.is_none() && !body.is_empty() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                        album_id = v.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
                    }
                }
                if let Some(id) = album_id {
                    progress::request_cancel(&id);
                    let conn = rusqlite::Connection::open(config.database_file_path.clone());
                    if let Ok(conn) = conn {
                        let _ = conn.execute("DELETE FROM album_state WHERE id = ?1 AND state = 'Requested'", rusqlite::params![id]);
                    }
                    let payload = serde_json::json!({"status":"ok"});
                    let _ = request.respond(json_response(payload.to_string()));
                } else {
                    let _ = request.respond(error_response(StatusCode(400), "missing id"));
                }
            }
            (Method::Post, path) if path.starts_with("/api/remove") => {
                let mut album_id: Option<String> = None;
                let query = url.splitn(2, '?').nth(1).unwrap_or("");
                if !query.is_empty() {
                    let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
                    album_id = params.get("id").cloned();
                }
                if album_id.is_none() && !body.is_empty() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                        album_id = v.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
                    }
                }
                if let Some(id) = album_id {
                    let conn = rusqlite::Connection::open(config.database_file_path.clone());
                    if let Ok(conn) = conn {
                        let mut album_path: Option<String> = None;
                        let _ = conn.query_row(
                            "SELECT path FROM album_state WHERE id = ?1",
                            rusqlite::params![id],
                            |row| {
                                let p: String = row.get(0)?;
                                album_path = Some(p);
                                Ok(())
                            }
                        );
                        if let Some(path_str) = album_path {
                            let base = std::path::PathBuf::from(config.library_path.clone());
                            let dir = base.join(&path_str);
                            if dir.exists() {
                                let _ = std::fs::remove_dir_all(&dir);
                            }
                        }
                        let _ = conn.execute("DELETE FROM album_state WHERE id = ?1", rusqlite::params![id]);
                    }
                    let payload = serde_json::json!({"status":"ok"});
                    let _ = request.respond(json_response(payload.to_string()));
                } else {
                    let _ = request.respond(error_response(StatusCode(400), "missing id"));
                }
            }
            (Method::Get, "/ui/now") => {
                let now = chrono::Local::now().format("%H:%M:%S").to_string();
                let _ = request.respond(html_response(ui::render_now(&now)));
            }
            (Method::Get, "/ui/stats") => {
                let registry = SQLiteRegistry::init(config.database_file_path.clone());
                let html = match registry.get_stats() {
                    Ok(stats) => {
                        let dto = StatsDto::from(stats);
                        let view = ui::StatsView {
                            album_requested: dto.album_requested,
                            album_processing: dto.album_processing,
                            album_synchronized: dto.album_synchronized,
                            count_total: dto.count_total,
                        };
                        ui::render_stats_html(&view)
                    }
                    Err(_) => "<div class=stat>Stats unavailable</div>".to_string(),
                };
                let _ = request.respond(html_response(html));
            }
            (Method::Get, "/ui/progress") => {
                let snapshot = progress::snapshot();
                let mut items: Vec<ui::ProgressView> = Vec::new();
                items.reserve(snapshot.len());
                for p in snapshot.values() {
                    items.push(ui::ProgressView::from(p));
                }
                let html = ui::render_progress_html(&items);
                let _ = request.respond(html_response(html));
            }
            (Method::Get, path) if path.starts_with("/ui/requested") => {
                let html = match list_albums(&config.database_file_path, "Requested", 10000, 0) {
                    Ok(albums) => {
                        let rows: Vec<AlbumRow> = albums.into_iter().map(AlbumRow::from).collect();
                        ui::render_albums_tbody("Requested", &rows)
                    },
                    Err(_) => String::new(),
                };
                let _ = request.respond(html_response(html));
            }
            (Method::Get, path) if path.starts_with("/ui/albums") => {
                let html = match list_albums(&config.database_file_path, "Synchronized", 10000, 0) {
                    Ok(albums) => {
                        let rows: Vec<AlbumRow> = albums.into_iter().map(AlbumRow::from).collect();
                        ui::render_albums_tbody("Synchronized", &rows)
                    },
                    Err(_) => String::new(),
                };
                let _ = request.respond(html_response(html));
            }
            _ => {
                let _ = request.respond(error_response(StatusCode(404), "Not Found"));
            }
        }
    }
}

#[derive(Serialize)]
struct StatsDto {
    album_requested: u64,
    album_processing: u64,
    album_synchronized: u64,
    count_total: u64,
}

impl From<crate::library::registry::RegistryStats> for StatsDto {
    fn from(value: crate::library::registry::RegistryStats) -> Self {
        StatsDto {
            album_requested: value.album_requested,
            album_processing: value.album_processing,
            album_synchronized: value.album_synchronized,
            count_total: value.count_total,
        }
    }
}

#[derive(Serialize)]
struct AlbumViewRow {
    id: String,
    state: String,
    path: String,
    cover_url: Option<String>,
    updated_at: String,
    artist: Option<String>,
    title: Option<String>,
}

fn list_albums(db_path: &str, state: &str, limit: i64, offset: i64) -> Result<Vec<AlbumViewRow>, Box<dyn std::error::Error>> {
    let conn = rusqlite::Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, state, path, cover_url, updated_at, details FROM album_state WHERE state = ?1 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt.query_map(rusqlite::params![state, limit, offset], |row| {
        let details: String = row.get(5)?;
        let parsed: serde_json::Value = serde_json::from_str(&details).unwrap_or(serde_json::Value::Null);
        Ok(AlbumViewRow {
            id: row.get::<_, i64>(0)?.to_string(),
            state: row.get(1)?,
            path: row.get(2)?,
            cover_url: row.get(3)?,
            updated_at: row.get(4)?,
            artist: parsed.get("artist").and_then(|a| a.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string()),
            title: parsed.get("title").and_then(|n| n.as_str()).map(|s| s.to_string()),
        })
    })?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn json_response(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response = Response::from_string(body);
    response.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap());
    response
}

fn html_response(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response = Response::from_string(body);
    response.add_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap());
    response
}

fn error_response(status: StatusCode, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let payload = serde_json::json!({"error": message});
    let mut response = Response::from_string(payload.to_string());
    response.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap());
    response.with_status_code(status)
}
