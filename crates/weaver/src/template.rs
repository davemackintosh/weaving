use std::{ffi::OsStr, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{config::TemplateLang, normalize_line_endings};

#[derive(Debug, Serialize, Deserialize)]
pub struct Template {
    pub at_path: PathBuf,
    pub contents: String,
    pub template_language: TemplateLang,
}

impl Template {
    pub fn new_from_path(path: PathBuf) -> Self {
        let contents_result = std::fs::read_to_string(&path);

        if contents_result.is_err() {
            dbg!("error reading file: {}", contents_result.err());
            panic!("failed to read '{}'", path.display());
        }

        let parseable = normalize_line_endings(contents_result.as_ref().unwrap().as_bytes());

        Self {
            at_path: path.clone(),
            contents: parseable,
            template_language: match path.clone().extension().and_then(OsStr::to_str).unwrap() {
                "liquid" => TemplateLang::Liquid,
                _ => panic!(
                    "Not sure what templating engine to use for this file. {}",
                    path.display()
                ),
            },
        }
    }

    pub fn new_from_string(contents: String, template_language: TemplateLang) -> Self {
        Self {
            at_path: "".into(),
            contents,
            template_language,
        }
    }
}
