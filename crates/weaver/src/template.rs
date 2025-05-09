use std::path::PathBuf;

#[derive(Debug)]
pub struct Template {
    pub at_path: PathBuf,
    pub contents: String,
}

impl Template {
    pub fn new_from_path(path: PathBuf) -> Self {
        let contents_result = std::fs::read_to_string(&path);

        if contents_result.is_err() {
            dbg!("error reading file: {}", contents_result.err());
            panic!("failed to read '{}'", path.display());
        }

        Self {
            at_path: path,
            contents: contents_result.unwrap(),
        }
    }
}
