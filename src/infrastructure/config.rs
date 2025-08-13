use dotenv_config::EnvConfig;

#[derive(Debug, Clone, EnvConfig)]
pub struct Config {
    #[env_config(name = "LIBRARY_PATH", default = "/music", help = "music library location, used as output - need write permission")]
    pub library_path: String,
    #[env_config(name = "SESSION_STORE_PATH", default = "/config", help = "keep streaming session - need write permission")]
    pub session_store_path: String,
    #[env_config(name = "DATABASE_FILE_PATH", default = "./library.db", help = "path to SQLite database file")]
    pub database_file_path: String,
    #[env_config(name = "SYNC_INTERVAL_SECONDS", default = "300", help = "Interval between sync cycles in seconds (no cron)")]
    pub sync_interval_seconds: Option<u64>,
    #[env_config(name = "HTTP_ENABLED", default = "true", help = "enable embedded HTTP server")]
    pub http_enabled: Option<bool>,
    #[env_config(name = "HTTP_HOST", default = "0.0.0.0", help = "HTTP bind host")]
    pub http_host: Option<String>,
    #[env_config(name = "HTTP_PORT", default = "8080", help = "HTTP bind port")]
    pub http_port: Option<u16>,
}