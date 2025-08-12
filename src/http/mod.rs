use std::collections::HashMap;
use std::io::Read;

use log::{error, info};
use serde::Serialize;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::infrastructure::config::Config;
use crate::library::registry::{SQLiteRegistry, FavouriteAlbums};

pub mod progress;

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
                let response = Response::from_string(index_html())
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
                // support id via query string or JSON body {"id":"..."}
                let mut album_id: Option<String> = None;
                // try query param first
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
                    // request cancel for any active processing
                    progress::request_cancel(&id);
                    // also remove from DB if it is still in Requested state so it won't be picked up later
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
                // support id via query string or JSON body {"id":"..."}
                let mut album_id: Option<String> = None;
                // try query param first
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
                    // open DB
                    let conn = rusqlite::Connection::open(config.database_file_path.clone());
                    if let Ok(conn) = conn {
                        // try to load album path (only for synchronized items we expect to be in library)
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

fn error_response(status: StatusCode, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let payload = serde_json::json!({"error": message});
    let mut response = Response::from_string(payload.to_string());
    response.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap());
    response.with_status_code(status)
}

fn index_html() -> String {
    // Minimal single-file frontend with simple polling and progress bars
    let html = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <title>Melody Magnet</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 0; background: #0f172a; color: #e2e8f0; }
    header { padding: 16px; background: #111827; display: flex; justify-content: space-between; align-items: center; }
    h1 { margin: 0; font-size: 18px; }
    .container { padding: 16px; display: grid; grid-template-columns: 1fr; gap: 16px; }
    .card { background: #1f2937; border-radius: 8px; padding: 16px; box-shadow: 0 1px 2px rgba(0,0,0,.4); }
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 12px; }
    .stat { font-size: 14px; }
    .progress { background: #334155; border-radius: 6px; overflow: hidden; height: 10px; }
    .bar { background: #22c55e; height: 100%; width: 0%; transition: width .4s ease; }
    table { width: 100%; border-collapse: collapse; }
    th, td { padding: 8px; border-bottom: 1px solid #334155; text-align: left; font-size: 14px; }
    .muted { color: #94a3b8; font-size: 12px; }
    .badge { padding: 2px 8px; border-radius: 999px; background: #0ea5e9; font-size: 12px; }
  </style>
</head>
<body>
  <header>
    <h1>Melody Magnet</h1>
    <div class="muted">Live: <span id="last-update">-</span></div>
  </header>
  <div class="container">
    <div class="grid">
      <div class="card" id="stats">
        <div class="stat">Requested: <b id="stat-requested">0</b></div>
        <div class="stat">Processing: <b id="stat-processing">0</b></div>
        <div class="stat">Synchronized: <b id="stat-synced">0</b></div>
        <div class="stat muted">Total: <b id="stat-total">0</b></div>
      </div>
      <div class="card">
        <h3 style="margin-top:0">Active Downloads</h3>
        <div id="progress-list"></div>
      </div>
      <div class="card">
        <h3 style="margin-top:0">Requested</h3>
        <table>
          <thead><tr><th>ID</th><th>Artist</th><th>Title</th><th>Path</th><th></th></tr></thead>
          <tbody id="requested"></tbody>
        </table>
      </div>
    </div>
    <div class="card">
      <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;">
        <h3 style="margin:0">Library (Synchronized)</h3>
        <span class="badge" id="filter">state=Synchronized</span>
      </div>
      <table>
        <thead><tr><th>ID</th><th>Artist</th><th>Title</th><th>Path</th><th>Updated</th><th></th></tr></thead>
        <tbody id="albums"></tbody>
      </table>
    </div>
  </div>
  <script>
    const fmtPct = (n) => `${Math.max(0, Math.min(100, n)).toFixed(0)}%`;
    async function fetchJson(url){ const r = await fetch(url); return await r.json(); }
    async function postJson(url, payload){
      const r = await fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
      return await r.json();
    }
    async function cancelTask(id){
      await postJson('/api/cancel', { id });
      refresh();
    }
    async function removeAlbum(id){
      if (!confirm('Remove this album from library? This will delete files.')) return;
      await postJson('/api/remove', { id });
      refresh();
    }
    async function refresh(){
      const [stats, progress, albums, requested] = await Promise.all([
        fetchJson('/api/stats').catch(()=>null),
        fetchJson('/api/progress').catch(()=>({})),
        fetchJson('/api/albums?state=Synchronized&limit=10000').catch(()=>[]),
        fetchJson('/api/albums?state=Requested&limit=10000').catch(()=>[]),
      ]);
      if (stats){
        document.getElementById('stat-requested').textContent = stats.album_requested;
        document.getElementById('stat-processing').textContent = stats.album_processing;
        document.getElementById('stat-synced').textContent = stats.album_synchronized;
        document.getElementById('stat-total').textContent = stats.count_total;
      }
      const list = document.getElementById('progress-list');
      list.innerHTML = '';
      Object.values(progress).forEach(p => {
        const pct = p.total_tracks > 0 ? (p.tracks_downloaded / p.total_tracks)*100 : 0;
        const el = document.createElement('div');
        el.innerHTML = `
          <div style="margin:6px 0; display:flex; justify-content:space-between; align-items:center; gap:8px;">
            <div><b>${p.artist} - ${p.title}</b> <span class="muted">(${p.tracks_downloaded}/${p.total_tracks})</span></div>
            <button onclick="cancelTask('${p.album_id}')" style="background:#ef4444;color:white;border:none;border-radius:6px;padding:4px 8px;cursor:pointer">Cancel</button>
          </div>
          <div class="progress"><div class="bar" style="width:${fmtPct(pct)}"></div></div>
          <div class="muted">${p.state} • ${p.bytes_downloaded} bytes</div>
        `;
        list.appendChild(el);
      });
      const tbody = document.getElementById('albums');
      tbody.innerHTML = '';
      albums.forEach(a => {
        const tr = document.createElement('tr');
        tr.innerHTML = `<td>${a.id}</td><td>${a.artist ?? ''}</td><td>${a.title ?? ''}</td><td>${a.path}</td><td class="muted">${a.updated_at}</td><td><button onclick="removeAlbum('${a.id}')" style="background:#ef4444;color:white;border:none;border-radius:6px;padding:4px 8px;cursor:pointer">Remove</button></td>`;
        tbody.appendChild(tr);
      });
      const reqBody = document.getElementById('requested');
      reqBody.innerHTML = '';
      requested.forEach(a => {
        const tr = document.createElement('tr');
        tr.innerHTML = `<td>${a.id}</td><td>${a.artist ?? ''}</td><td>${a.title ?? ''}</td><td>${a.path}</td><td><button onclick="cancelTask('${a.id}')" style="background:#ef4444;color:white;border:none;border-radius:6px;padding:4px 8px;cursor:pointer">Cancel</button></td>`;
        reqBody.appendChild(tr);
      });
      document.getElementById('last-update').textContent = new Date().toLocaleTimeString();
    }
    refresh();
    setInterval(refresh, 1000);
  </script>
</body>
</html>"#;
    html.to_string()
}
