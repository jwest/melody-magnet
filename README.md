# 🎵🧲 MelodyMagnet (in development)

Create your private music library by downloading music from streaming services.

## Frontend (Web UI) and HTTP API
MelodyMagnet includes a minimal embedded HTTP server that serves a small web dashboard and a few JSON endpoints. This lets you monitor sync/download progress and browse recently synchronized albums.

- Default URL: http://localhost:8080
- Frontend UI: GET /
- Health: GET /api/health
- Stats: GET /api/stats
- Albums list: GET /api/albums?state=Synchronized&limit=20&offset=0
- Progress snapshot: GET /api/progress

Notes:
- The current UI is a single page with simple polling every ~2s (no SSE/WebSocket yet).
- Album list defaults to state=Synchronized; you can change the state via query string to Requested or Processing.

### Enable/Configure the HTTP server
You can control the embedded HTTP server via environment variables. Defaults are shown in parentheses.

- HTTP_ENABLED (true): set to false to disable the HTTP server entirely.
- HTTP_HOST (0.0.0.0): host/interface to bind.
- HTTP_PORT (8080): port to listen on.

Example .env file:

```
LIBRARY_PATH=./library
SESSION_STORE_PATH=.
DATABASE_FILE_PATH=./library.db
TIME_ZONE=Europe/London
CRON_TAB_DEFINITION=* * * * * *
HTTP_ENABLED=true
HTTP_HOST=0.0.0.0
HTTP_PORT=8080
```

### Run locally
1. Ensure you have a valid Tidal session token (the app will manage `tidal_session.json` in SESSION_STORE_PATH once you login/refresh via backend logic).
2. Build and run:
   - cargo run --release
3. Open the dashboard:
   - http://localhost:8080

### Docker / Compose
A simple docker-compose_example.yml is provided. Ensure the HTTP port is published:

```yaml
services:
  melody-magnet:
    image: melody-magnet:latest
    environment:
      - HTTP_ENABLED=true
      - HTTP_HOST=0.0.0.0
      - HTTP_PORT=8080
    ports:
      - "8080:8080"
    volumes:
      - ./library:/music
      - ./:/config
```

Then open http://localhost:8080.

### What you can do from the UI
- See aggregate stats (Requested, Processing, Synchronized).
- Watch active downloads with per‑album progress bars (track count and total bytes saved).
- Browse a table of synchronized albums (first 20 by updated time). Use query params in the URL bar to adjust state, limit, and offset if needed (e.g., `/api/albums?state=Processing&limit=50&offset=0`).

### Limitations (current phase)
- The UI uses polling, so updates appear up to ~2 seconds later.
- Progress is approximate: bytes are counted after each track save.
- No authentication on the HTTP server; expose only on trusted networks.

## Roadmap / TODO
- Runner refactoring for cron and async download requests.
- Favourite artists' albums and tracks download.
- HTTP server improvements: Tidal login flow, trigger sync, live updates via SSE, richer browsing and search.

## Disclaimer
- Private use only.
- Need a Tidal-HIFI subscription.
- You should not use this method to distribute or pirate music.
- It may be illegal to use this in your country, so be informed.