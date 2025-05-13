use document::Document;
use futures::future::join_all;
use glob::glob;
use renderers::{MarkdownRenderer, TemplateRenderer};
use std::{
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

// Define a struct to hold the data needed for rendering templates
// These will be Arc'd to be shared across tasks
#[derive(Clone)] // Derive Clone to easily pass it into async move blocks
pub struct RenderContext {
    pub config: Arc<WeaverConfig>,
    pub documents: Arc<Vec<Arc<Mutex<Document>>>>,
    pub templates: Arc<Vec<Arc<Mutex<Template>>>>,
    pub tags: Vec<String>,
    pub routes: Vec<String>,
}

impl RenderContext {
    pub async fn to_liquid_data(&self) -> liquid::Object {
        // 1. Config
        let mut liquid_object = liquid::object!({
            "config": *self.config.clone(),
        });

        // 2. Documents
        let document_futures: Vec<_> = self
            .documents
            .iter()
            .map(|doc_arc| {
                let doc_arc_clone = Arc::clone(doc_arc);
                async move {
                    let doc_guard = doc_arc_clone.lock().await;
                    liquid::model::to_value(&*doc_guard).expect("Failed to serialize document")
                }
            })
            .collect();
        let liquid_documents: Vec<liquid::model::Value> = join_all(document_futures).await;
        liquid_object.insert(
            "documents".into(),
            liquid::model::Value::array(liquid_documents),
        );

        // 3. Templates (assuming Template or TemplateRenderer can be serialized)
        let template_futures: Vec<_> = self
            .templates
            .iter()
            .map(|tmpl_arc| {
                let tmpl_arc_clone = Arc::clone(tmpl_arc);
                async move {
                    let tmpl_guard = tmpl_arc_clone.lock().await;
                    liquid::model::to_value(&*tmpl_guard).expect("Failed to serialize template")
                }
            })
            .collect();
        let liquid_templates: Vec<liquid::model::Value> = join_all(template_futures).await;
        liquid_object.insert(
            "templates".into(),
            liquid::model::Value::array(liquid_templates),
        );

        // 4. Tags (Vec<String> converts easily)
        liquid_object.insert(
            "tags".into(),
            liquid::model::to_value(&self.tags).expect("Failed to serialize tags"),
        );

        // 5. Routes (Vec<String> converts easily)
        liquid_object.insert(
            "routes".into(),
            liquid::model::to_value(&self.routes).expect("Failed to serialize routes"),
        );

        liquid_object
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
                // Filter out '.' and '..' components, and root/prefix components
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

    pub async fn build(&self) -> Result<(), BuildError> {
        dbg!("Starting build process");

        // --- Prepare shared data wrapped in Arc ---
        let config_arc = Arc::clone(&self.config);
        let documents_arc = Arc::new(self.documents.clone());
        let templates_arc = Arc::new(self.templates.clone());
        let tags_vec = self.tags.clone();
        let routes_vec = self.routes.clone();

        // Create the RenderContext template
        let render_context_template = RenderContext {
            config: Arc::clone(&config_arc), // Clone Arcs for the context template
            documents: Arc::clone(&documents_arc),
            templates: Arc::clone(&templates_arc),
            routes: routes_vec,
            tags: tags_vec,
        };
        let task_context = Arc::new(render_context_template.to_liquid_data().await);
        // --- End Prepare shared data ---

        // Create a vector to hold our futures
        let mut tasks = vec![];

        // Spawn tasks for building documents
        for document in &self.documents {
            let document_arc = Arc::clone(document); // Clone the Arc for this specific document
            let context = Arc::clone(&task_context);
            let doc_task = tokio::spawn(async move {
                let md_renderer = MarkdownRenderer::new(document_arc);
                md_renderer.render(&context).await
            });
            tasks.push(doc_task);
        }

        // Spawn tasks for parsing templates
        for template in &self.templates {
            let template_arc = Arc::clone(template); // Clone the Arc for this specific template
            let context = Arc::clone(&task_context);
            let template_task = tokio::spawn(async move {
                let renderer = TemplateRenderer::new(template_arc);

                renderer.render(&context).await
            });
            tasks.push(template_task);
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
