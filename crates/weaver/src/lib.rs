use document::Document;
use futures::future::join_all;
use glob::glob;
use liquid::model::KString;
use renderers::{globals::LiquidGlobals, ContentRenderer, MarkdownRenderer, WritableFile};
use routes::route_from_path;
use std::{collections::HashMap, error::Error, fmt::Display, io::prelude::*, sync::Arc};
use template::Template;
use tokio::sync::Mutex;

use config::{TemplateLang, WeaverConfig};

/// Weaver is the library that powers weaving, as in Hugo Weaving. It does nothing but compile
/// templates and markdown files to their static counterparts.
/// There is zero requirement for a config file at all, defaults are used- however specifying
/// content locations can vary from user to user so afford them the opportunity to do so.
pub mod config;
pub mod document;
pub mod filters;
pub mod renderers;
pub mod routes;
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

    pub fn scan_content(&mut self) -> &mut Self {
        dbg!("searching for content");
        for entry in glob(format!("{}/**/*.md", self.config.content_dir).as_str())
            .expect("Failed to read glob pattern")
        {
            match entry {
                Ok(path) => {
                    let mut doc = Document::new_from_path(path.clone());
                    self.tags.append(&mut doc.metadata.tags);
                    self.routes.push(route_from_path(
                        self.config.content_dir.clone().into(),
                        path.clone(),
                    ));
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

    async fn map_from_documents(&self) -> HashMap<KString, Arc<Mutex<Document>>> {
        let mut map = HashMap::new();

        for document in self.documents.iter() {
            let doc_guard = document.lock().await;
            map.insert(
                KString::from(route_from_path(
                    self.config.content_dir.clone().into(),
                    doc_guard.at_path.clone().into(),
                )),
                document.clone(),
            );
        }

        map
    }

    async fn write_result_to_system(&self, target: WritableFile) -> Result<(), BuildError> {
        println!("Writing file {:#?}", target);
        //let mut file = File::create(target.path).unwrap();
        //file.write_all(target.contents.as_bytes()).unwrap();

        Ok(())
    }

    pub async fn build(&self) -> Result<(), BuildError> {
        dbg!("Starting build process");

        let templates_arc = Arc::new(self.templates.clone());
        let mut tasks = vec![];

        for document in &self.documents {
            let document_arc = Arc::clone(document);
            let templates = Arc::clone(&templates_arc);
            let config = Arc::clone(&self.config);
            let mut globals =
                LiquidGlobals::new(document_arc.clone(), &self.map_from_documents().await).await;
            let doc_task = tokio::spawn(async move {
                let md_renderer = MarkdownRenderer::new(document_arc, templates, config);
                md_renderer.render(&mut globals).await
            });
            tasks.push(doc_task);
        }

        // Wait for all tasks to complete and collect their results
        // The outer Result is JoinError, the inner Result is from your async function
        let results: Vec<Result<Result<WritableFile, BuildError>, tokio::task::JoinError>> =
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

                    // TODO: Cache output file paths and remove files that aren't part of the
                    // output.
                    self.write_result_to_system(inner_result?).await?;
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
