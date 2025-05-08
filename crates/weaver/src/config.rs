use serde::Deserialize;

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum TemplateLang {
    #[default]
    Liquid,
}

#[derive(Deserialize, Debug)]
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
            template_dir: "templats".into(),
            templating_language: TemplateLang::Liquid,
        }
    }
}

impl WeaverConfig {
    pub fn new() -> Self {
        let inst = Self::default();

        let config_file_result = std::fs::read_to_string(format!("{}/weaving.toml", inst.base_dir));

        if config_file_result.is_err() {
            dbg!(format!(
                "Didn't find a weaving.toml at '{}'",
                &inst.base_dir
            ));
            dbg!(format!("using default config {:#?}", &inst));
            return Self {
                version: "1".into(),
                base_dir: inst.base_dir.clone(),
                content_dir: format!("{}/{}", &inst.base_dir, inst.content_dir),
                base_url: inst.base_url,
                includes_dir: format!("{}/{}", &inst.base_dir, inst.includes_dir),
                public_dir: format!("{}/{}", &inst.base_dir, inst.public_dir),
                build_dir: format!("{}/{}", &inst.base_dir, inst.build_dir),
                templating_language: TemplateLang::Liquid,
            };
        } else {
            dbg!(format!(
                "Found config file at '{}/weaving.toml'",
                inst.base_dir
            ));
        }

        let user_supplied_config: WeaverConfig =
            toml::from_str(config_file_result.unwrap().as_str()).unwrap();
        dbg!(format!(
            "using supplied config {:#?}",
            &user_supplied_config
        ));

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
        }
    }

    pub fn new_from_path(base_path: String) -> Self {
        let config_file_result = std::fs::read_to_string(format!("{}/weaving.toml", base_path));

        if config_file_result.is_err() {
            panic!("Didn't find a weaving.toml at '{}'", &base_path);
        } else {
            dbg!(format!("Found config file at '{}/weaving.toml'", base_path));
        }

        let user_supplied_config: WeaverConfig =
            toml::from_str(config_file_result.unwrap().as_str()).unwrap();
        dbg!(format!(
            "using supplied config {:#?}",
            &user_supplied_config
        ));

        Self {
            version: user_supplied_config.version,
            base_dir: base_path.clone(),
            content_dir: format!("{}/{}", &base_path, user_supplied_config.content_dir),
            base_url: user_supplied_config.base_url,
            includes_dir: format!("{}/{}", &base_path, user_supplied_config.includes_dir),
            public_dir: format!("{}/{}", &base_path, user_supplied_config.public_dir),
            build_dir: format!("{}/{}", &base_path, user_supplied_config.build_dir),
            templating_language: user_supplied_config.templating_language,
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
        let base_path = format!("{}/tests/empty_config", base_path_wd);
        let config = WeaverConfig::new_from_path(base_path.clone());

        assert_eq!(config.base_dir, base_path);
        assert_eq!(config.content_dir, format!("{}/content", base_path));
        assert_eq!(config.includes_dir, format!("{}/includes", base_path));
        assert_eq!(config.public_dir, format!("{}/public", base_path));
        assert_eq!(config.build_dir, format!("{}/site", base_path));
        assert_eq!(config.base_url, "localhost:8080");
    }

    #[test]
    fn test_with_filled_config_file() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/tests/custom_config", base_path_wd);
        let config = WeaverConfig::new_from_path(base_path.clone());

        assert_eq!(config.base_dir, base_path);
        assert_eq!(config.content_dir, format!("{}/web-content", base_path));
        assert_eq!(config.includes_dir, format!("{}/partials", base_path));
        assert_eq!(config.public_dir, format!("{}/static", base_path));
        assert_eq!(config.build_dir, format!("{}/site", base_path));
        assert_eq!(config.base_url, "localhost:9090");
    }

    #[test]
    fn test_with_partial_config_file() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/tests/partial_config", base_path_wd);
        let config = WeaverConfig::new_from_path(base_path.clone());

        assert_eq!(config.base_dir, base_path);
        assert_eq!(config.content_dir, format!("{}/content", base_path));
        assert_eq!(config.includes_dir, format!("{}/partials", base_path));
        assert_eq!(config.public_dir, format!("{}/static", base_path));
        assert_eq!(config.build_dir, format!("{}/site", base_path));
        assert_eq!(config.base_url, "localhost:8080");
    }
}
