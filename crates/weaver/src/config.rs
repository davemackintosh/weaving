use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum TemplateLang {
    #[default]
    Liquid,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ImageConfig {
    pub quality: u8,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self { quality: 83 }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct WeaverConfig {
    pub version: String,
    pub base_dir: String,
    pub content_dir: String,
    pub base_url: String,
    pub includes_dir: String,
    pub public_dir: String,
    pub template_dir: String,
    pub build_dir: String,
    pub templating_language: TemplateLang,
    pub image_config: ImageConfig,
}

impl Default for WeaverConfig {
    fn default() -> Self {
        let base_path = std::env::var_os("WEAVING_BASE_PATH")
            .unwrap_or(std::env::current_dir().unwrap().as_os_str().to_os_string())
            .to_str()
            .unwrap()
            .to_string();

        Self {
            version: "1".into(),
            base_dir: base_path.clone(),
            content_dir: "content".into(),
            base_url: "localhost:8080".into(),
            includes_dir: "includes".into(),
            public_dir: "public".into(),
            build_dir: "site".into(),
            template_dir: "templates".into(),
            templating_language: TemplateLang::Liquid,
            image_config: Default::default(),
        }
    }
}

impl WeaverConfig {
    pub fn new() -> Self {
        let inst = Self::default();

        let config_file_result = std::fs::read_to_string(format!("{}/weaving.toml", inst.base_dir));

        if config_file_result.is_err() {
            return Self {
                version: "1".into(),
                base_dir: inst.base_dir.clone(),
                content_dir: format!("{}/{}", &inst.base_dir, inst.content_dir),
                template_dir: format!("{}/{}", &inst.base_dir, inst.template_dir),
                base_url: inst.base_url,
                includes_dir: format!("{}/{}", &inst.base_dir, inst.includes_dir),
                public_dir: format!("{}/{}", &inst.base_dir, inst.public_dir),
                build_dir: format!("{}/{}", &inst.base_dir, inst.build_dir),
                templating_language: TemplateLang::Liquid,
                image_config: inst.image_config,
            };
        }
        let user_supplied_config: WeaverConfig =
            toml::from_str(config_file_result.unwrap().as_str()).unwrap();

        Self {
            version: user_supplied_config.version,
            base_dir: user_supplied_config.base_dir.clone(),
            content_dir: format!(
                "{}/{}",
                &user_supplied_config.base_dir, user_supplied_config.content_dir
            ),
            base_url: user_supplied_config.base_url,
            includes_dir: format!(
                "{}/{}",
                &user_supplied_config.base_dir, user_supplied_config.includes_dir
            ),
            public_dir: format!(
                "{}/{}",
                &user_supplied_config.base_dir, user_supplied_config.public_dir
            ),
            build_dir: format!(
                "{}/{}",
                &user_supplied_config.base_dir, user_supplied_config.build_dir
            ),
            template_dir: format!(
                "{}/{}",
                &user_supplied_config.base_dir, user_supplied_config.template_dir
            ),
            templating_language: user_supplied_config.templating_language,
            image_config: user_supplied_config.image_config,
        }
    }

    pub fn new_from_path(base_path: PathBuf) -> Self {
        // Remove any trailing slash.
        let safe_path = base_path.to_str().unwrap();
        let config_file_result = std::fs::read_to_string(format!("{}/weaving.toml", safe_path));

        if config_file_result.is_err() {
            return Self::new();
        }

        let user_supplied_config: WeaverConfig =
            toml::from_str(config_file_result.unwrap().as_str()).unwrap();

        Self {
            version: user_supplied_config.version,
            base_dir: safe_path.to_string(),
            content_dir: format!("{}/{}", &safe_path, user_supplied_config.content_dir),
            base_url: user_supplied_config.base_url,
            includes_dir: format!("{}/{}", &safe_path, user_supplied_config.includes_dir),
            public_dir: format!("{}/{}", &safe_path, user_supplied_config.public_dir),
            build_dir: format!("{}/{}", &safe_path, user_supplied_config.build_dir),
            template_dir: format!("{}/{}", &safe_path, user_supplied_config.template_dir),
            templating_language: user_supplied_config.templating_language,
            image_config: user_supplied_config.image_config,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_defaultyness() {
        let base_path = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let config = WeaverConfig::new();

        assert_eq!(config.base_dir, base_path);
        assert_eq!(config.content_dir, format!("{}/content", base_path));
        assert_eq!(config.includes_dir, format!("{}/includes", base_path));
        assert_eq!(config.public_dir, format!("{}/public", base_path));
        assert_eq!(config.build_dir, format!("{}/site", base_path));
        assert_eq!(config.base_url, "localhost:8080");
    }

    #[test]
    fn test_with_empty_config_file() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/test_fixtures/config/empty_config", base_path_wd);
        let config = WeaverConfig::new_from_path(base_path.clone().into());

        assert_eq!(config.base_dir, base_path);
        assert_eq!(config.content_dir, format!("{}/content", base_path));
        assert_eq!(config.includes_dir, format!("{}/includes", base_path));
        assert_eq!(config.public_dir, format!("{}/public", base_path));
        assert_eq!(config.build_dir, format!("{}/site", base_path));
        assert_eq!(config.base_url, "localhost:8080");
    }

    #[test]
    fn test_with_filled_config_file() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/config/custom_config", base_path_wd);
        let config = WeaverConfig::new_from_path(base_path.clone().into());

        assert_eq!(config.base_dir, base_path);
        assert_eq!(config.content_dir, format!("{}/content", base_path));
        assert_eq!(config.includes_dir, format!("{}/partials", base_path));
        assert_eq!(config.public_dir, format!("{}/static", base_path));
        assert_eq!(config.build_dir, format!("{}/site", base_path));
        assert_eq!(config.base_url, "localhost:9090");
    }

    #[test]
    fn test_with_partial_config_file() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/config/partial_config", base_path_wd);
        let config = WeaverConfig::new_from_path(base_path.clone().into());

        assert_eq!(config.base_dir, base_path);
        assert_eq!(config.content_dir, format!("{}/content", base_path));
        assert_eq!(config.includes_dir, format!("{}/partials", base_path));
        assert_eq!(config.public_dir, format!("{}/static", base_path));
        assert_eq!(config.build_dir, format!("{}/site", base_path));
        assert_eq!(config.base_url, "localhost:8080");
    }
}
