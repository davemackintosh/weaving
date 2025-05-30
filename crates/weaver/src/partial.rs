use std::path::PathBuf;

use regex::RegexBuilder;
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

        let re = RegexBuilder::new(r"<([a-zA-Z][a-zA-Z0-9]*)([^>]*)>")
            .case_insensitive(true)
            .build()
            .expect("Failed to compile regex for HTML tags");

        let original_content = normalize_line_endings(contents_result.as_ref().unwrap().as_bytes());
        let contents = re.replace_all(&original_content, "$0\n").to_string();

        Self {
            at_path: path.display().to_string(),
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            contents,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_partial_whitespace() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/liquid/partials", base_path_wd);
        let partial = Partial::new_from_path(format!("{}/test.liquid", base_path).into());

        assert_eq!("<div>\n\n\ttest\n</div>\n", partial.contents,);
    }
}
