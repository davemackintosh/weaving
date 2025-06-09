use chrono::{DateTime, Local};
use gray_matter::{Matter, engine::YAML};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use toml::Value;

use crate::{document_toc::toc_from_document, normalize_line_endings};

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
pub struct Heading {
    pub depth: u8,
    pub text: String,
    pub slug: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Document {
    pub at_path: String,
    pub metadata: BaseMetaData,
    pub markdown: String,
    pub excerpt: Option<String>,
    pub html: Option<String>,
    pub toc: Vec<Heading>,
    pub emit: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(default)]
pub struct BaseMetaData {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub keywords: Vec<String>,
    pub template: String,
    pub emit: bool,

    #[serde(flatten)]
    pub user: HashMap<String, Value>,
}

impl Default for BaseMetaData {
    fn default() -> Self {
        Self {
            title: Default::default(),
            tags: Default::default(),
            description: Default::default(),
            keywords: Default::default(),
            template: "default".into(),
            emit: true,
            user: Default::default(),
        }
    }
}

impl Document {
    pub fn new_from_path(path: PathBuf) -> Self {
        let contents_result = std::fs::read_to_string(&path);
        let file_meta = std::fs::metadata(&path).unwrap();

        if contents_result.is_err() {
            dbg!("error reading file: {}", contents_result.err());
            panic!("failed to read '{}'", path.display());
        }

        let matter = Matter::<YAML>::new();
        let parseable = normalize_line_endings(contents_result.as_ref().unwrap().as_bytes());
        println!("{}", &parseable);
        let parse_result = matter.parse(&parseable);
        let base_metadata_opt = match parse_result.data {
            Some(data) => data.deserialize::<BaseMetaData>(),
            None => Ok(BaseMetaData::default()),
        };

        if base_metadata_opt.is_err() {
            eprintln!(
                "error parsing '{}': {:?}",
                &path.display(),
                base_metadata_opt.err()
            );
            return Self::default();
        }

        let mut base_metadata = base_metadata_opt.unwrap();

        // If there's no published in the base_metadata, we will use the file's created at meta.
        if !base_metadata.user.contains_key("published") {
            // TODO: Fix the unwraps here.
            base_metadata.user.insert(
                "published".into(),
                Value::from(DateTime::<Local>::from(file_meta.created().unwrap()).to_string()),
            );
            base_metadata.user.insert(
                "last_updated".into(),
                base_metadata.user.get("published").unwrap().clone(),
            );
        } else {
            base_metadata.user.insert(
                "last_updated".into(),
                Value::from(DateTime::<Local>::from(file_meta.modified().unwrap()).to_string()),
            );
        }

        let should_emit = base_metadata.clone().emit;

        Self {
            at_path: path.display().to_string(),
            metadata: base_metadata,
            markdown: parse_result.content.clone(),
            excerpt: parse_result.excerpt,
            emit: should_emit,
            toc: toc_from_document(parse_result.content.as_str()),
            ..Default::default()
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
        let time: DateTime<Local> = Local::now();
        let expected = BaseMetaData {
            tags: vec!["1".into()],
            keywords: vec!["2".into()],
            title: "test".into(),
            description: "test".into(),
            published: Some(time),
            last_updated: Some(time),
            user: HashMap::new(),
            emit: true,
            template: "default".into(),
        };

        assert_eq!(expected.tags, document.metadata.tags);
        assert_eq!(expected.keywords, document.metadata.keywords);
        assert_eq!(expected.title, document.metadata.title);
        assert_eq!(expected.description, document.metadata.description);
        assert_eq!(expected.user, document.metadata.user);
        assert_eq!(expected.emit, document.metadata.emit);
        assert_eq!(expected.template, document.metadata.template);
    }
}
