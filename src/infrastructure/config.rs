use dotenv_config::EnvConfig;

#[derive(Debug, EnvConfig)]
pub struct Config {
    #[env_config(name = "LIBRARY_PATH", default = "/music", help = "music library location, used as output - need write permission")]
    pub library_path: String,
    #[env_config(name = "SESSION_STORE_PATH", default = "/config", help = "keep streaming session - need write permission")]
    pub session_store_path: String,
    #[env_config(name = "DATABASE_FILE_PATH", default = "./library.db", help = "path to SQLite database file")]
    pub database_file_path: String,
    #[env_config(name = "TIME_ZONE", default = "Europe/London", help = "Time zone definition for proper CRON job execution")]
    pub time_zone: String,
    #[env_config(name = "CRON_TAB_DEFINITION", default = "* * * * * *", help = "Cron tab definition")]
    pub cron_tab_definition: String,
}