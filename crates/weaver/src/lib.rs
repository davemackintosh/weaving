use document::Document;
use futures::future::join_all;
use glob::glob;
use std::{error::Error, sync::Arc};
use template::Template;
use tokio::sync::Mutex;

use config::{TemplateLang, WeaverConfig};

/// Weaver is the library that powers weaving, as in Hugo Weaving. It does nothing but compile
/// templates and markdown files to their static counterparts.
/// There is zero requirement for a config file at all, defaults are used- however specifying
/// content locations can vary from user to user so afford them the opportunity to do so.
pub mod config;
pub mod document;
pub mod template;

pub struct Weaver {
    pub config: Arc<WeaverConfig>,
    pub tags: Vec<String>,
    pub templates: Vec<Arc<Mutex<Template>>>,
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
                Ok(path) => self
                    .templates
                    .push(Arc::new(Mutex::new(Template::new_from_path(path)))),
                Err(e) => panic!("{:?}", e),
            }
        }

        self
    }

    async fn build_document(
        &self,
        document: Arc<Mutex<Document>>,
    ) -> Result<(), markdown::message::Message> {
        let mut doc = document.lock().await;
        let html = markdown::to_html_with_options(&doc.markdown, &markdown::Options::gfm())?;

        doc.html = Some(html);

        Ok(())
    }

    async fn parse_template(&self, template: Arc<Mutex<Template>>) -> Result<(), Box<dyn Error>> {
        match self.config.templating_language {
            TemplateLang::Liquid => {}
        }
    }

    pub async fn build(&self) -> Result<(), Box<dyn Error>> {
        dbg!("Starting build process");

        // Create a vector to hold our asynchronous tasks (futures)
        let mut tasks = vec![];

        // Spawn tasks for building documents
        for document in &self.documents {
            let doc_task = tokio::spawn(self.build_document(Arc::clone(document)));
            tasks.push(doc_task);
        }

        // Spawn tasks for parsing templates
        for template in &self.templates {
            let template_task = tokio::spawn(self.parse_template(Arc::clone(template)));
            tasks.push(template_task);
        }

        // Wait for all tasks to complete and collect their results
        let results: Vec<Result<Result<(), Box<dyn std::error::Error>>, tokio::task::JoinError>> =
            join_all(tasks).await;

        // Check for errors in the results
        for result in results {
            match result {
                Ok(inner_result) => {
                    if let Err(e) = inner_result {
                        eprintln!("Error during build: {}", e);
                        // Depending on your error handling strategy, you might want to
                        // return the first error encountered, or collect all errors.
                        return Err(e);
                    }
                }
                Err(e) => {
                    eprintln!("Task join error: {}", e);
                    // Handle the case where a spawned task itself failed to run
                    return Err(Box::new(e));
                }
            }
        }

        dbg!("Build process finished successfully");
        Ok(())
    }
}
