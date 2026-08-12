use std::collections::HashMap;
use std::path::PathBuf;

/// Placeholder API key used when an engine has not been configured yet.
pub const PLACEHOLDER_KEY: &str = "YOUR_OPENAI_API_KEY";

/// Configuration for a single image-generation engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

impl EngineConfig {
    /// True when the engine has a usable (non-placeholder) API key.
    pub fn key_set(&self) -> bool {
        !self.api_key.is_empty() && self.api_key != PLACEHOLDER_KEY
    }
}

/// Global settings loaded from the environment (and `.env` file).
#[derive(Debug, Clone)]
pub struct Settings {
    pub output_dir: PathBuf,
    pub default_engine: Option<String>,
    pub engines: HashMap<String, EngineConfig>,
}

impl Settings {
    /// Load settings from environment variables / `.env` file.
    ///
    /// Supported variables (mirroring the Python original):
    /// - `IMAGAI__OUTPUT_DIR`
    /// - `IMAGAI__DEFAULT_ENGINE`
    /// - `IMAGAI__ENGINES__<NAME>__API_KEY`
    /// - `IMAGAI__ENGINES__<NAME>__BASE_URL`
    /// - `IMAGAI__ENGINES__<NAME>__MODEL`
    pub fn load() -> Self {
        let mut settings = Settings {
            output_dir: PathBuf::from("generated_images"),
            default_engine: None,
            engines: HashMap::new(),
        };

        // Load `.env` from current dir, walking up to the repo root.
        for dir in [std::env::current_dir().ok(), dir_of_repo_root()]
            .into_iter()
            .flatten()
        {
            if dir.join(".env").is_file() {
                let _ = dotenvy::from_path(dir.join(".env"));
                break;
            }
        }

        for (key, value) in std::env::vars() {
            if let Some(rest) = key.strip_prefix("IMAGAI__") {
                if let Some(engines_rest) = rest.strip_prefix("ENGINES__") {
                    let parts: Vec<&str> = engines_rest.split("__").collect();
                    if parts.len() >= 2 {
                        let engine_name = parts[0].to_lowercase();
                        let field = parts[1].to_lowercase();
                        let entry =
                            settings
                                .engines
                                .entry(engine_name)
                                .or_insert_with(|| EngineConfig {
                                    api_key: "dummy".to_string(),
                                    base_url: None,
                                    model: None,
                                });
                        match field.as_str() {
                            "api_key" => entry.api_key = value,
                            "base_url" => entry.base_url = Some(value),
                            "model" => entry.model = Some(value),
                            _ => {}
                        }
                    }
                } else if rest == "OUTPUT_DIR" {
                    settings.output_dir = PathBuf::from(value);
                } else if rest == "DEFAULT_ENGINE" {
                    settings.default_engine = Some(value);
                }
            }
        }

        // Ensure the output directory exists.
        if let Err(e) = std::fs::create_dir_all(&settings.output_dir) {
            eprintln!("Warning: could not create output dir: {e}");
        }

        settings
    }

    /// Return configured engine names (sorted).
    pub fn engine_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.engines.keys().cloned().collect();
        names.sort();
        names
    }

    /// Find an engine by name (case-insensitive).
    pub fn get_engine(&self, name: &str) -> Option<&EngineConfig> {
        self.engines.get(name).or_else(|| {
            self.engines
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v)
        })
    }
}

/// Walk up from cwd looking for a directory containing a `.git` folder,
/// used to locate the repo root `.env`.
fn dir_of_repo_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
