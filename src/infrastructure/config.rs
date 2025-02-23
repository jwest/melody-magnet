use dotenv_config::EnvConfig;

#[derive(Debug, EnvConfig)]
pub struct Config {
    #[env_config(name = "LIBRARY_PATH", default = "/music", help = "music library location, used as output - need write permission")]
    pub library_path: String,
    #[env_config(name = "SESSION_STORE_PATH", default = "/config", help = "keep streaming session - need write permission")]
    pub session_store_path: String,
}