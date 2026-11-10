use log::{debug, error, info, trace};
use serde::Serialize;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "config.yml";

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct AppConfig {
    pub version: (u64, u64),
    pub remote_url: String,
    pub user: String,
    pub password: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: (1, 0),
            remote_url: "cr.tidop.es".to_string(),
            user: String::new(),
            password: String::new(),
        }
    }
}

#[allow(unused)]
impl AppConfig {
    #[must_use]
    pub fn is_missing_credentials(&self) -> bool {
        self.user.is_empty() || self.password.is_empty()
    }

    pub fn registry_credentials(&self) -> (String, String) {
        (self.user.clone(), self.password.clone())
    }

    // Using https://docs.rs/eframe/latest/eframe/fn.storage_dir.html
    /// Deserialize config from file. Inits the file if not found.
    pub fn get_config() -> Result<Self, ()> {
        let config_dir = get_config_dir_path()?;
        info!("Using config dir: {}", config_dir.display());
        match std::fs::create_dir_all(&config_dir) {
            Ok(()) => {
                trace!("Dir exists");
            }
            Err(x) => {
                error!("Error when creating dir: {x:?}");
                return Err(());
            }
        }
        let file_path = get_config_file_path()?;
        info!("Using config file: {}", file_path.display());
        // If the config file is missing, create the dir and file with AppConfig::default();
        if !file_path.exists() {
            info!("Config file does not exist");
            self::overwrite_default_config()?;
        }

        // Read and parse config file
        debug!("Reading config file...");
        let Ok(yaml_input) = std::fs::read_to_string(file_path) else {
            error!("Fatal: can't read config file");
            return Err(());
        };
        let mut config = match serde_saphyr::from_str(&yaml_input) {
            Ok(x) => x,
            Err(x) => {
                error!("Error when parsing config file: {x:?}");
                self::overwrite_default_config();
                AppConfig::default()
            }
        };
        trace!("{config:?}");
        Ok(config)
    }

    /// Save current config into file
    pub fn serialize_config(&self) -> Result<(), ()> {
        let file_path = get_config_file_path()?;
        let mut output_yaml = String::new();
        let mut ser = serde_saphyr::Serializer::new(&mut output_yaml);
        if let Err(x) = self.serialize(&mut ser) {
            error!("Error when serializing config: {x:?}");
            return Err(());
        }
        trace!("Serialized YAML: {output_yaml}");
        if let Err(x) = std::fs::write(&file_path, output_yaml) {
            error!("Error when writing config file: {x:?}");
            return Err(());
        }
        Ok(())
    }
}

// This does not create the dir. No trailing separator
pub fn get_config_dir_path() -> Result<PathBuf, ()> {
    let Some(config_dir) = eframe::storage_dir("recoge") else {
        return Err(());
    };
    Ok(config_dir)
}

// This does not create the file
pub fn get_config_file_path() -> Result<PathBuf, ()> {
    let config_dir = get_config_dir_path()?;
    let mut file_path: std::path::PathBuf = config_dir.clone();
    let mut p = file_path.into_os_string();
    p.push(std::path::MAIN_SEPARATOR.to_string());
    p.push(CONFIG_FILE_NAME);
    file_path = p.into();
    Ok(file_path)
}

pub fn overwrite_default_config() -> Result<(), ()> {
    let file_path = get_config_file_path()?;
    info!("Writing default config file");
    let mut output_yaml = String::new();
    let mut ser = serde_saphyr::Serializer::new(&mut output_yaml);
    let config = AppConfig::default();
    if let Err(x) = config.serialize(&mut ser) {
        error!("Error when serializing default config: {x:?}");
        return Err(());
    }
    trace!("Serialized default YAML: {output_yaml}");
    if let Err(x) = std::fs::write(&file_path, output_yaml) {
        error!("Error when writing default config file: {x:?}");
        return Err(());
    }
    info!(
        "Config file created OK for v{}.{}",
        config.version.0, config.version.1
    );
    Ok(())
}
