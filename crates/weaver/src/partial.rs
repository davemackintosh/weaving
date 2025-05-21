use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::normalize_line_endings;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Partial {
    pub name: String,
    pub at_path: String,
    pub contents: String,
}

impl Partial {
    pub fn new_from_path(path: PathBuf) -> Self {
        let contents_result = std::fs::read_to_string(&path);

        if contents_result.is_err() {
            dbg!("error reading file: {}", contents_result.err());
            panic!("failed to read '{}'", path.display());
        }

        let contents = normalize_line_endings(contents_result.as_ref().unwrap().as_bytes());

        Self {
            at_path: path.display().to_string(),
            name: path.file_name().unwrap().display().to_string(),
            contents,
        }
    }
}
