use std::path::PathBuf;

#[derive(Debug)]
pub struct Document {
    pub at_path: String,
    pub tags: Vec<String>,
    pub markdown: String,
    pub html: Option<String>,
}

impl Document {
    pub fn new_from_path(path: PathBuf) -> Self {
        let contents_result = std::fs::read_to_string(&path);

        if contents_result.is_err() {
            dbg!("error reading file: {}", contents_result.err());
            panic!("failed to read '{}'", path.display());
        }

        Self {
            at_path: path.to_str().unwrap().to_string(),
            tags: vec![],
            markdown: contents_result.unwrap(),
            html: None,
        }
    }
}
