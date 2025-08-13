use crate::http::progress::AlbumProgress;

// UI module: only pure HTML rendering utilities and view structs.
// No database, filesystem, or network logic here.

pub fn index_html() -> &'static str {
    // Loaded from a separate template file at compile time.
    include_str!("index.html")
}

pub fn render_now(now: &str) -> String { now.to_string() }

pub struct StatsView {
    pub album_requested: u64,
    pub album_processing: u64,
    pub album_synchronized: u64,
    pub count_total: u64,
}

pub fn render_stats_html(stats: &StatsView) -> String {
    format!(
        "<div class=stat>Requested: <b>{}</b></div>
         <div class=stat>Processing: <b>{}</b></div>
         <div class=stat>Synchronized: <b>{}</b></div>
         <div class=stat muted>Total: <b>{}</b></div>",
        stats.album_requested, stats.album_processing, stats.album_synchronized, stats.count_total
    )
}

pub fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

// View struct for progress rendering. We reuse AlbumProgress fields but decouple from Snapshot source.
pub struct ProgressView<'a> {
    pub album_id: &'a str,
    pub artist: &'a str,
    pub title: &'a str,
    pub total_tracks: u32,
    pub tracks_downloaded: u32,
    pub bytes_downloaded: u64,
    pub state: &'a str,
}

impl<'a> From<&'a AlbumProgress> for ProgressView<'a> {
    fn from(p: &'a AlbumProgress) -> Self {
        ProgressView {
            album_id: &p.album_id,
            artist: &p.artist,
            title: &p.title,
            total_tracks: p.total_tracks,
            tracks_downloaded: p.tracks_downloaded,
            bytes_downloaded: p.bytes_downloaded,
            state: &p.state,
        }
    }
}

pub fn render_progress_html(progress: &[ProgressView<'_>]) -> String {
    let mut out = String::new();
    for p in progress {
        let pct = if p.total_tracks > 0 { (p.tracks_downloaded as f32 / p.total_tracks as f32) * 100.0 } else { 0.0 };
        out.push_str(&format!(
            "<div style=\"margin:6px 0; display:flex; justify-content:space-between; align-items:center; gap:8px;\">\
                <div><b>{artist} - {title}</b> <span class=muted>({done}/{total})</span></div>\
                <button hx-post=\"/api/cancel?id={id}\" class=btn-danger>Cancel</button>\
             </div>\
             <div class=progress><div class=bar style=\"width:{pct:.0}%\"></div></div>\
             <div class=muted>{state} • {bytes} bytes</div>",
            artist = escape_html(p.artist),
            title = escape_html(p.title),
            done = p.tracks_downloaded,
            total = p.total_tracks,
            id = p.album_id,
            pct = pct,
            state = p.state,
            bytes = p.bytes_downloaded
        ));
    }
    out
}

// View struct for album table rows
pub struct AlbumRow {
    pub id: String,
    pub state: String,
    pub path: String,
    pub cover_url: Option<String>,
    pub updated_at: String,
    pub artist: Option<String>,
    pub title: Option<String>,
}

impl From<super::AlbumViewRow> for AlbumRow {
    fn from(v: super::AlbumViewRow) -> Self {
        AlbumRow {
            id: v.id,
            state: v.state,
            path: v.path,
            cover_url: v.cover_url,
            updated_at: v.updated_at,
            artist: v.artist,
            title: v.title,
        }
    }
}


pub fn render_albums_tbody(state: &str, albums: &[AlbumRow]) -> String {
    let mut out = String::new();
    for a in albums {
        let artist = a.artist.as_deref().unwrap_or("");
        let title = a.title.as_deref().unwrap_or("");
        if state == "Requested" {
            out.push_str(&format!(
                "<tr><td>{id}</td><td>{artist}</td><td>{title}</td><td>{path}</td><td class=muted>-</td><td><button hx-post=\"/api/cancel?id={id}\" class=btn-danger>Cancel</button></td></tr>",
                id = a.id,
                artist = escape_html(artist),
                title = escape_html(title),
                path = escape_html(&a.path)
            ));
        } else {
            out.push_str(&format!(
                "<tr><td>{id}</td><td>{artist}</td><td>{title}</td><td>{path}</td><td class=muted>{updated}</td><td><button hx-confirm=\"Remove this album from library? This will delete files.\" hx-post=\"/api/remove?id={id}\" class=btn-danger>Remove</button></td></tr>",
                id = a.id,
                artist = escape_html(artist),
                title = escape_html(title),
                path = escape_html(&a.path),
                updated = escape_html(&a.updated_at)
            ));
        }
    }
    out
}
