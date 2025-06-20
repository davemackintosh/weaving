use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use glob::glob;
use liquid::model::KString;
use owo_colors::OwoColorize;

use crate::{
    BuildError,
    config::WeaverConfig,
    renderers::{WritableFile, globals::LiquidGlobalsPage},
};

use super::WeaverTask;

#[derive(Default)]
pub struct CleanBuildDirTask;

unsafe impl Send for CleanBuildDirTask {}
unsafe impl Sync for CleanBuildDirTask {}

#[async_trait]
impl WeaverTask for CleanBuildDirTask {
    async fn run(
        &self,
        config: Arc<WeaverConfig>,
        content: &Arc<HashMap<KString, LiquidGlobalsPage>>,
    ) -> Result<Option<WritableFile>, BuildError> {
        println!("scanning for and removing old content from build directory");
        let target = config.build_dir.clone();
        let current_content: HashMap<String, bool> = content
            .iter()
            .map(|(_k, v)| (v.route.to_string(), true))
            .collect();
        let mut deletable_files = vec![];
        let mut deletable_dirs = vec![];

        dbg!(&current_content);

        for entry in glob(format!("{}/**/*", target).as_str()).expect("Failed to read glob pattern")
        {
            match entry {
                Ok(path) => {
                    let public_dir = PathBuf::from(config.public_dir.clone());
                    let route_path = path.strip_prefix(config.build_dir.clone()).unwrap();
                    let public_dir_name = public_dir.iter().next_back().unwrap();
                    let route = PathBuf::from(route_path);

                    // Skip anything in the public directory (for now)
                    if route.starts_with(public_dir_name) {
                        continue;
                    }

                    // We also want to leave content from other internal tasks like sitemap and
                    // atom.
                    if route.ends_with("sitemap.xml") || route.ends_with("atom.xml") {
                        continue;
                    }

                    let comparitor = if path.is_dir() {
                        format!("/{}/", route_path.to_str().unwrap(),)
                    } else {
                        format!("/{}", route_path.to_str().unwrap())
                    };

                    dbg!(&comparitor);
                    if !current_content.contains_key(&comparitor) {
                        if path.is_dir() {
                            deletable_dirs.push(path);
                        } else {
                            deletable_files.push(path);
                        }
                    }
                }
                Err(e) => panic!("{:?}", e),
            }
        }

        dbg!(&deletable_files);

        deletable_files
            .iter()
            .for_each(|p| match fs::remove_file(p) {
                Ok(..) => {}
                Err(e) => {
                    eprintln!(
                        "{}",
                        format!("failed to delete file {} because {}", p.display(), e).red()
                    );
                }
            });
        deletable_dirs.iter().for_each(|p| match fs::remove_dir(p) {
            Ok(..) => {}
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("failed to delete directory {} because {}", p.display(), e).red()
                );
            }
        });

        Ok(None)
    }
}
