use document::Document;
use futures::future::join_all;
use glob::glob;
use liquid::model::KString;
use renderers::{MarkdownRenderer, globals::LiquidGlobals};
use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    path::{Path, PathBuf},
    sync::Arc,
};
use template::Template;
use tokio::sync::Mutex;

use config::{TemplateLang, WeaverConfig};

/// Weaver is the library that powers weaving, as in Hugo Weaving. It does nothing but compile
/// templates and markdown files to their static counterparts.
/// There is zero requirement for a config file at all, defaults are used- however specifying
/// content locations can vary from user to user so afford them the opportunity to do so.
pub mod config;
pub mod document;
pub mod renderers;
pub mod template;

#[derive(Debug)]
pub enum BuildError {
    Err(String),
}

impl Error for BuildError {}

impl Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Err(err) => write!(f, "{}", err),
        }
    }
}

pub struct Weaver {
    pub config: Arc<WeaverConfig>,
    pub tags: Vec<String>,
    pub routes: Vec<String>,
    pub templates: Vec<Arc<Mutex<Template>>>,
    pub documents: Vec<Arc<Mutex<Document>>>,
}

impl Weaver {
    pub fn new(base_path: String) -> Self {
        Self {
            config: Arc::new(WeaverConfig::new_from_path(base_path)),
            tags: vec![],
            routes: vec![],
            templates: vec![],
            documents: vec![],
        }
    }

    fn route_from_path(&self, path: PathBuf) -> String {
        // Ensure content_dir is an absolute path for robust stripping
        let content_dir = PathBuf::from(&self.config.content_dir);

        // 1. Strip the base content directory prefix
        let relative_path = match path.strip_prefix(&content_dir) {
            Ok(p) => p,
            Err(_) => {
                // This should ideally not happen if paths are correctly managed.
                // Or it means the path is outside the content directory.
                // Handle this error case appropriately, e.g., panic, return an error, or log.
                // For now, let's just panic and halt the build because this won't output something
                // visible or usable to anyone.
                panic!(
                    "Warning: Path {:?} is not within content directory {:?}",
                    path, content_dir
                );
            }
        };

        let mut route_parts: Vec<String> = relative_path
            .components()
            .filter_map(|c| {
                // Filter out relative path components, and root/prefix components
                match c {
                    std::path::Component::Normal(os_str) => {
                        Some(os_str.to_string_lossy().into_owned())
                    }
                    _ => None,
                }
            })
            .collect();

        // 2. Handle file extension and "pretty URLs"
        if let Some(last_segment) = route_parts.pop() {
            let original_filename_path = Path::new(&last_segment);

            if original_filename_path.file_stem().is_some() {
                let stem = original_filename_path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy();

                if stem == "index" {
                    // If it's an index file, the URI is just its parent directory
                    // The parent directory is already represented by the remaining route_parts
                    // So, no need to add "index" to the route.
                    // Example: content/posts/index.md -> /posts/
                } else {
                    // For other files, use the stem as the segment and add a trailing slash
                    // Example: content/posts/my-post.md -> /posts/my-post/
                    route_parts.push(stem.into_owned());
                }
            }
        }

        // 3. Join parts with forward slashes and ensure leading/trailing slashes
        let mut route = format!("/{}", route_parts.join("/"));

        // Ensure trailing slash for directories, unless it's the root '/'
        if route.len() > 1 {
            route.push('/');
        }

        // Special case for root index.md (e.g., content/index.md -> /)
        // If the original relative_path was just "index.md"
        if relative_path.to_string_lossy() == "index.md" {
            route = "/".to_string();
        }

        route
    }

    pub fn scan_content(&mut self) -> &mut Self {
        dbg!("searching for content");
        for entry in glob(format!("{}/**/*.md", self.config.content_dir).as_str())
            .expect("Failed to read glob pattern")
        {
            match entry {
                Ok(path) => {
                    let mut doc = Document::new_from_path(path.clone());
                    self.tags.append(&mut doc.metadata.tags);
                    self.routes.push(self.route_from_path(path.clone()));
                    self.documents.push(Arc::new(Mutex::new(doc)))
                }
                Err(e) => panic!("{:?}", e),
            }
        }

        self
    }

    pub fn scan_templates(&mut self) -> &mut Self {
        dbg!("searching for templates");
        let extension = match self.config.templating_language {
            TemplateLang::Liquid => ".liquid",
        };
        for entry in glob(format!("{}/**/*{}", self.config.content_dir, extension).as_str())
            .expect("Failed to read glob pattern")
        {
            match entry {
                Ok(pathbuf) => self
                    .templates
                    .push(Arc::new(Mutex::new(Template::new_from_path(pathbuf)))),
                Err(e) => panic!("{:?}", e),
            }
        }

        self
    }

    async fn map_from_documents(
        &self,
    ) -> HashMap<KString, Arc<tokio::sync::Mutex<crate::Document>>> {
        let mut map = HashMap::new();

        for document in self.documents.iter() {
            let doc_guard = document.lock().await;
            map.insert(
                KString::from(self.route_from_path(doc_guard.at_path.clone().into())),
                document.clone(),
            );
        }

        map
    }

    pub async fn build(&self) -> Result<(), BuildError> {
        dbg!("Starting build process");

        let templates_arc = Arc::new(self.templates.clone());

        // Create a vector to hold our futures
        let mut tasks = vec![];

        // Spawn tasks for building documents
        for document in &self.documents {
            let document_arc = Arc::clone(document);
            let templates = Arc::clone(&templates_arc);
            let mut globals =
                LiquidGlobals::new(document_arc.clone(), &self.map_from_documents().await).await;
            let doc_task = tokio::spawn(async move {
                let md_renderer = MarkdownRenderer::new(document_arc, templates);
                md_renderer.render(&mut globals).await
            });
            tasks.push(doc_task);
        }

        // Wait for all tasks to complete and collect their results
        // The outer Result is JoinError, the inner Result is from your async function
        let results: Vec<Result<Result<String, BuildError>, tokio::task::JoinError>> =
            join_all(tasks).await;

        // Check for errors in the results
        for result in results {
            match result {
                Ok(inner_result) => {
                    if let Err(e) = inner_result {
                        eprintln!("Error during task execution: {}", e);
                        // Depending on your error handling, you might want to
                        // return the first error or collect all errors.
                        return Err(e);
                    }
                }
                Err(e) => {
                    eprintln!("Task join error: {}", e);
                    // Convert the JoinError into your BuildError type
                    return Err(BuildError::Err(e.to_string()));
                }
            }
        }

        dbg!("Build process finished successfully");
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn test_route_from_path() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/test_fixtures/config", base_path_wd);
        let inst = Weaver::new(format!("{}/custom_config", base_path));

        assert_eq!(
            "/blog/post1/",
            inst.route_from_path(format!("{}/blog/post1.md", inst.config.content_dir).into())
        );
    }

    #[test]
    #[should_panic]
    fn test_content_out_of_path() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/test_fixtures/config", base_path_wd);
        let inst = Weaver::new(format!("{}/custom_config", base_path));
        inst.route_from_path("madeup/blog/post1.md".into());
    }
}
