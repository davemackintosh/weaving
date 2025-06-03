use config::{TemplateLang, WeaverConfig};
use document::Document;
use futures::future::join_all;
use glob::glob;
use liquid::model::KString;
use owo_colors::OwoColorize;
use partial::Partial;
use renderers::{
    ContentRenderer, MarkdownRenderer, WritableFile,
    globals::{LiquidGlobals, LiquidGlobalsPage},
};
use routes::route_from_path;
use std::{collections::HashMap, error::Error, fmt::Display, path::PathBuf, sync::Arc};
use template::Template;
use tokio::sync::Mutex;

/// Weaver is the library that powers weaving, as in Hugo Weaving. It does nothing but compile
/// templates and markdown files to their static counterparts.
/// There is zero requirement for a config file at all, defaults are used- however specifying
/// content locations can vary from user to user so afford them the opportunity to do so.
pub mod config;
pub mod document;
pub mod document_toc;
pub mod filters;
pub mod partial;
pub mod renderers;
pub mod routes;
pub mod template;

use std::fs;
use std::path::Path;

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<WritableFile, BuildError> {
    fs::create_dir_all(&dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name())).unwrap();
        }
    }
    Ok(WritableFile {
        contents: "".into(),
        path: "".into(),
        emit: true,
    })
}

// Helper function to normalize line endings in a byte vector
pub fn normalize_line_endings(bytes: &[u8]) -> String {
    let s = str::from_utf8(bytes).expect("Invalid UTF-8 in WritableFile content");
    // Replace all CRLF (\r\n) with LF (\n)
    s.replace("\r\n", "\n")
}

#[derive(Debug)]
pub enum BuildError {
    Err(String),
    IoError(String),
    GlobError(String),
    DocumentError(String),
    TemplateError(String),
    RouteError(String),
    RenderError(String),
    JoinError(String),
}

impl Error for BuildError {}

impl Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Err(msg) => write!(f, "Generic Build Error: {}", msg),
            BuildError::IoError(msg) => write!(f, "I/O Error: {}", msg),
            BuildError::GlobError(msg) => write!(f, "Glob Error: {}", msg),
            BuildError::DocumentError(msg) => write!(f, "Document Error: {}", msg),
            BuildError::TemplateError(msg) => write!(f, "Template Error: {}", msg),
            BuildError::RouteError(msg) => write!(f, "Route Error: {}", msg),
            BuildError::RenderError(msg) => write!(f, "Render Error: {}", msg),
            BuildError::JoinError(msg) => write!(f, "Task Join Error: {}", msg),
        }
    }
}

impl From<tokio::task::JoinError> for BuildError {
    fn from(err: tokio::task::JoinError) -> Self {
        BuildError::JoinError(err.to_string())
    }
}

pub struct Weaver {
    pub config: Arc<WeaverConfig>,
    pub tags: Vec<String>,
    pub routes: Vec<String>,
    pub templates: Vec<Arc<Mutex<Template>>>,
    pub documents: Vec<Arc<Mutex<Document>>>,
    pub partials: Vec<Partial>,
    all_documents_by_route: HashMap<KString, Arc<Mutex<Document>>>,
}

impl Weaver {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            config: Arc::new(WeaverConfig::new(base_path)),
            tags: vec![],
            routes: vec![],
            templates: vec![],
            partials: vec![],
            documents: vec![],
            all_documents_by_route: HashMap::new(),
        }
    }

    pub fn scan_content(&mut self) -> &mut Self {
        for entry in glob(format!("{}/**/*.md", self.config.content_dir).as_str())
            .expect("Failed to read glob pattern")
        {
            match entry {
                Ok(path) => {
                    let mut doc = Document::new_from_path(path.clone());

                    self.tags.append(&mut doc.metadata.tags);
                    // Assuming route_from_path is correct and returns String
                    let route = route_from_path(self.config.content_dir.clone().into(), path);
                    self.routes.push(route.clone());

                    let doc_arc_mutex = Arc::new(Mutex::new(doc));
                    self.documents.push(Arc::clone(&doc_arc_mutex));

                    self.all_documents_by_route
                        .insert(KString::from(route), doc_arc_mutex);
                }
                Err(e) => panic!("{:?}", e),
            }
        }

        self
    }

    pub fn scan_partials(&mut self) -> &mut Self {
        let extension = match self.config.templating_language {
            TemplateLang::Liquid => ".liquid",
        };
        println!(
            "Searching for {} templates in {}",
            &extension, &self.config.partials_dir
        );
        for entry in glob(format!("{}/**/*{}", self.config.partials_dir, extension).as_str())
            .expect("Failed to read glob pattern")
        {
            match entry {
                Ok(pathbuf) => {
                    println!(
                        "Found partial {}, registering {}",
                        pathbuf.display(),
                        pathbuf.file_name().unwrap().to_string_lossy()
                    );
                    let partial = Partial::new_from_path(pathbuf);
                    self.partials.push(partial);
                }
                Err(e) => panic!("{:?}", e), // Panics on glob iteration error
            }
        }

        self
    }

    pub fn scan_templates(&mut self) -> &mut Self {
        let extension = match self.config.templating_language {
            TemplateLang::Liquid => ".liquid",
        };
        for entry in glob(format!("{}/**/*{}", self.config.template_dir, extension).as_str())
            .expect("Failed to read glob pattern")
        {
            match entry {
                Ok(pathbuf) => self
                    .templates
                    .push(Arc::new(Mutex::new(Template::new_from_path(pathbuf)))), // Panics on file read/parse errors
                Err(e) => panic!("{:?}", e), // Panics on glob iteration error
            }
        }

        self
    }

    async fn write_result_to_system(&self, target: WritableFile) -> Result<(), BuildError> {
        let full_output_path = target.path.clone();

        // Ensure parent directories exist
        if let Some(parent) = full_output_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                BuildError::IoError(format!(
                    "Failed to create parent directories for {:?}: {}",
                    full_output_path, e
                ))
            })?;
        }

        println!("Writing {}", full_output_path.display().green());
        tokio::fs::write(&full_output_path, target.contents)
            .await
            .map_err(|e| {
                BuildError::IoError(format!(
                    "Failed to write file {:?}: {}",
                    full_output_path, e
                ))
            })?;

        Ok(())
    }

    // The main build orchestration function
    pub async fn build(&self) -> Result<(), BuildError> {
        let mut all_liquid_pages_map: HashMap<KString, LiquidGlobalsPage> = HashMap::new();
        let mut convert_tasks = vec![];

        for document_arc_mutex in self.documents.iter() {
            let doc_arc_mutex_clone = Arc::clone(document_arc_mutex);
            let config_arc = Arc::clone(&self.config);

            convert_tasks.push(tokio::spawn(async move {
                let doc_guard = doc_arc_mutex_clone.lock().await;
                let route = route_from_path(
                    config_arc.content_dir.clone().into(),
                    doc_guard.at_path.clone().into(),
                );
                let liquid_page = LiquidGlobalsPage::from(&*doc_guard);

                (KString::from(route), liquid_page)
            }));
        }

        let converted_pages: Vec<Result<(KString, LiquidGlobalsPage), tokio::task::JoinError>> =
            join_all(convert_tasks).await;

        for result in converted_pages {
            let (route, liquid_page) = result.map_err(|e| BuildError::JoinError(e.to_string()))?;
            all_liquid_pages_map.insert(route, liquid_page);
        }

        let all_liquid_pages_map_arc = Arc::new(all_liquid_pages_map);

        let templates_arc = Arc::new(self.templates.clone());
        let config_arc = Arc::clone(&self.config);
        let partials_arc = Arc::new(self.partials.clone());

        let mut tasks = vec![];

        for document_arc_mutex in &self.documents {
            let document_arc = Arc::clone(document_arc_mutex);

            let all_liquid_pages_map_clone = Arc::clone(&all_liquid_pages_map_arc);
            let mut globals =
                LiquidGlobals::new(Arc::clone(&document_arc), &all_liquid_pages_map_clone).await;

            let templates = Arc::clone(&templates_arc);
            let config = Arc::clone(&config_arc);
            let partials = Arc::clone(&partials_arc);

            let doc_task = tokio::spawn(async move {
                let md_renderer =
                    MarkdownRenderer::new(document_arc, templates, config, partials.to_vec());

                md_renderer.render(&mut globals, partials.to_vec()).await
            });

            tasks.push(doc_task);
        }

        let public_copy_task = tokio::spawn(async move {
            let config = Arc::clone(&config_arc);
            let folder_name = config
                .public_dir
                .clone()
                .split('/')
                .next_back()
                .unwrap()
                .to_string();
            let target = format!("{}/{}", config.build_dir.clone(), folder_name);

            if fs::exists(&config.public_dir)
                .expect("failed to check if there was a public directory")
            {
                println!("Copying {} to {}", config.public_dir.clone(), &target);

                copy_dir_all(config.public_dir.clone(), target)
            } else {
                Ok(WritableFile {
                    contents: "".into(),
                    path: "".into(),
                    emit: false,
                })
            }
        });

        tasks.push(public_copy_task);

        let render_results: Vec<Result<Result<WritableFile, BuildError>, tokio::task::JoinError>> =
            join_all(tasks).await; // Await all rendering tasks

        // Process the results of all rendering tasks
        for join_result in render_results {
            match join_result {
                Ok(render_result) => match render_result {
                    Ok(writable_file) => {
                        if writable_file.path.as_os_str() != "" && writable_file.emit {
                            self.write_result_to_system(writable_file).await?;
                        }
                    }
                    Err(render_error) => {
                        eprintln!("Rendering error: {}", render_error.red());
                        return Err(render_error);
                    }
                },
                Err(join_error) => {
                    eprintln!("Task join error: {}", join_error.red());
                    return Err(BuildError::JoinError(join_error.to_string()));
                }
            }
        }

        Ok(())
    }
}
