use std::fs;

use serde::Deserialize;

use crate::errors::LogAnalyzerErrors;

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Level,
    Service,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub delimiter: String,
    pub levels: Vec<String>,
    pub parallel: Option<bool>,
    pub target: Target,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            delimiter: String::from("|"),
            levels: vec!["INFO".to_string(), "WARN".to_string(), "ERROR".to_string()],
            parallel: None,
            target: Target::Level,
        }
    }
}

impl Config {
    pub fn read_from_file<'a>(file_path: &'a String) -> Result<Self, LogAnalyzerErrors<'a>> {
        let config_content = fs::read_to_string(file_path).map_err(|err| {
            LogAnalyzerErrors::ConfigReadError(
                format!("Failed to read config file `{}`", err),
                file_path,
            )
        })?;
        let config: Config = toml::from_str(&config_content).map_err(|err| {
            LogAnalyzerErrors::ConfigReadError(
                format!("Failed to parse config file `{}`", err),
                file_path,
            )
        })?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.delimiter, "|");
        assert_eq!(cfg.target, Target::Level);
        assert!(cfg.levels.contains(&"ERROR".to_string()));
    }

    #[test]
    fn test_toml_deserialization() {
        let toml_str = r#"
            delimiter = ","
            levels = ["DEBUG", "INFO"]
            target = "service"
            parallel = false
        "#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.delimiter, ",");
        assert_eq!(cfg.target, Target::Service);
        assert_eq!(cfg.parallel, Some(false));
    }
}
