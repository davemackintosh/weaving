use std::{collections::HashMap, path::PathBuf};

use gray_matter::{Matter, engine::YAML};
use serde::{Deserialize, Deserializer, Serialize};
use toml::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    pub at_path: String,
    pub metadata: BaseMetaData,
    pub markdown: String,
    pub excerpt: Option<String>,
    pub html: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct BaseMetaData {
    pub title: String,
    #[serde(deserialize_with = "deserialize_string_or_number_vec")]
    pub tags: Vec<String>,
    #[serde(deserialize_with = "deserialize_string_or_number_vec")]
    pub keywords: Vec<String>,

    #[serde(flatten)]
    custom: HashMap<String, Value>,
}

pub fn deserialize_string_or_number_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let values: Vec<Value> = Vec::deserialize(deserializer)?;
    let result: Vec<String> = values
        .into_iter()
        .map(|v| v.to_string().trim_matches('"').to_string())
        .collect();

    Ok(result)
}

impl Document {
    pub fn new_from_path(path: PathBuf) -> Self {
        let contents_result = std::fs::read_to_string(&path);

        if contents_result.is_err() {
            dbg!("error reading file: {}", contents_result.err());
            panic!("failed to read '{}'", path.display());
        }

        dbg!("read post {}", contents_result.as_ref().unwrap());

        // We parse both to a struct and to a plaid old data because it's helpful to have a
        // concrete type for required metadata. Title, tags, etc are required but we also want a hashmap of
        // sorts for any custom properties on a document.
        let matter = Matter::<YAML>::new();
        let parse_result = matter.parse(contents_result.as_ref().unwrap().as_str());
        let base_metadata_opt = parse_result
            .data
            .as_ref()
            .unwrap()
            .deserialize::<BaseMetaData>();

        if base_metadata_opt.is_err() {
            dbg!("error parsing: {}", base_metadata_opt.err());
            panic!("Failed to parse the frontmatter in {}", path.display());
        }

        let base_metadata = base_metadata_opt.unwrap();

        Self {
            at_path: path.display().to_string(),
            metadata: base_metadata,
            markdown: parse_result.content,
            excerpt: parse_result.excerpt,
            html: None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_document_loading() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/test_fixtures/markdown", base_path_wd);

        let document = Document::new_from_path(format!("{}/full_frontmatter.md", base_path).into());

        assert_eq!(
            BaseMetaData {
                tags: vec!["1".into()],
                keywords: vec!["2".into()],
                title: "test".into(),
                custom: HashMap::new()
            },
            document.metadata
        )
    }

    #[test]
    #[should_panic]
    fn test_bad_document_loading() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/test_fixtures/markdown", base_path_wd);

        Document::new_from_path(format!("{}/missing_frontmatter_keys.md", base_path).into());
    }
}
