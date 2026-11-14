use crate::constants::APP_NAME;
use directories::BaseDirs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub profiles_dir: PathBuf,
    pub state_file: PathBuf,
    /// The lighting presets the user saved, shared by every profile.
    pub light_presets_file: PathBuf,
    pub socket: PathBuf,
    pub log_file: PathBuf,
}

impl Paths {
    pub fn from_dirs(config: PathBuf, runtime_or_cache: PathBuf, state: PathBuf) -> Self {
        let config_dir = config.join(APP_NAME);
        Self {
            profiles_dir: config_dir.join("profiles"),
            state_file: config_dir.join("state.json"),
            light_presets_file: config_dir.join("light_presets.json"),
            socket: runtime_or_cache.join(APP_NAME).join("daemon.sock"),
            log_file: state.join(APP_NAME).join("opentartarus.log"),
            config_dir,
        }
    }

    pub fn from_env() -> Self {
        let base = BaseDirs::new().expect("home directory");
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| base.cache_dir().to_path_buf());
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| base.config_dir().to_path_buf());
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| base.home_dir().join(".local/state"));
        Self::from_dirs(config, runtime, state)
    }
}
