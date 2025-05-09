use document::Document;
use futures::future::join_all;
use glob::glob;
use renderers::TemplateRenderer;
use std::{error::Error, fmt::Display, sync::Arc};
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
        write!(f, "{}", self)
    }
}

// Define a struct to hold the data needed for rendering templates
// These will be Arc'd to be shared across tasks
#[derive(Clone)] // Derive Clone to easily pass it into async move blocks
pub struct RenderContext {
    pub config: Arc<WeaverConfig>,
    pub documents: Arc<Vec<Arc<Mutex<Document>>>>,
    pub templates: Arc<Vec<Arc<Mutex<TemplateRenderer>>>>,
    pub tags: Vec<String>,
}

pub struct Weaver {
    pub config: Arc<WeaverConfig>,
    pub tags: Vec<String>,
    pub templates: Vec<Arc<Mutex<TemplateRenderer>>>,
    pub documents: Vec<Arc<Mutex<Document>>>,
}

impl Weaver {
    pub fn new(base_path: String) -> Self {
        Self {
            config: Arc::new(WeaverConfig::new_from_path(base_path)),
            tags: vec![],
            templates: vec![],
            documents: vec![],
        }
    }

    pub fn scan_content(&mut self) -> &mut Self {
        dbg!("searching for content");
        for entry in glob(format!("{}/**/*.md", self.config.content_dir).as_str())
            .expect("Failed to read glob pattern")
        {
            match entry {
                Ok(path) => self
                    .documents
                    .push(Arc::new(Mutex::new(Document::new_from_path(path)))),
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
                    .push(Arc::new(Mutex::new(TemplateRenderer::new(pathbuf)))),
                Err(e) => panic!("{:?}", e),
            }
        }

        self
    }

    // Modified to not take &self
    async fn build_document(document: Arc<Mutex<Document>>) -> Result<(), BuildError> {
        let mut doc = document.lock().await;
        // Use the markdown body from the document struct (already separated by gray_matter)
        match markdown::to_html_with_options(&doc.markdown, &markdown::Options::gfm()) {
            Ok(html) => {
                dbg!(format!("Built document: {:?}", &html));
                doc.html = Some(html);
            }
            Err(err) => {
                // markdown crate uses markdown::message::Message for errors
                // Convert it to your BuildError
                return Err(BuildError::Err(err.reason));
            }
        }

        Ok(())
    }

    async fn render_template(
        template: Arc<Mutex<TemplateRenderer>>,
        context: RenderContext, // Pass the context by value (it contains Arcs)
    ) -> Result<(), BuildError> {
        let mut template_guard = template.lock().await;

        // Call the render method on TemplateRenderer, passing a reference to the context
        // You'll need to update TemplateRenderer::render to accept &RenderContext
        template_guard.render(&context).await; // Assuming render is async and takes &RenderContext
        Ok(())
    }

    pub async fn build(&self) -> Result<(), BuildError> {
        dbg!("Starting build process");

        // --- Prepare shared data wrapped in Arc ---
        let config_arc = Arc::clone(&self.config);
        let documents_arc = Arc::new(self.documents.clone());
        let templates_arc = Arc::new(self.templates.clone());
        let tags_vec = self.tags.clone();

        // Create the RenderContext template
        let render_context_template = RenderContext {
            config: Arc::clone(&config_arc), // Clone Arcs for the context template
            documents: Arc::clone(&documents_arc),
            templates: Arc::clone(&templates_arc),
            tags: tags_vec,
        };
        // --- End Prepare shared data ---

        // Create a vector to hold our asynchronous tasks (futures)
        let mut tasks = vec![];

        // Spawn tasks for building documents
        for document in &self.documents {
            let document_arc = Arc::clone(document); // Clone the Arc for this specific document
            let doc_task = tokio::spawn(async move {
                // The async move block takes ownership of document_arc
                Weaver::build_document(document_arc).await // Call the associated function
            });
            tasks.push(doc_task);
        }

        // Spawn tasks for parsing templates
        for template in &self.templates {
            let template_arc = Arc::clone(template); // Clone the Arc for this specific template
            let task_context = render_context_template.clone(); // Clone the context for this task
            let template_task = tokio::spawn(async move {
                // The async move block takes ownership of template_arc and task_context
                Weaver::render_template(template_arc, task_context).await // Call the associated function
            });
            tasks.push(template_task);
        }

        // Wait for all tasks to complete and collect their results
        // The outer Result is JoinError, the inner Result is from your async function
        let results: Vec<Result<Result<(), BuildError>, tokio::task::JoinError>> =
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
