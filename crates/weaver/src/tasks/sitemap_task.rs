use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    BuildError, Weaver,
    config::WeaverConfig,
    document::Document,
    filters::json::JSON,
    renderers::{WritableFile, globals::LiquidGlobals},
};

use super::WeaverTask;

#[derive(Default)]
pub struct SiteMapTask;

unsafe impl Send for SiteMapTask {}
unsafe impl Sync for SiteMapTask {}

#[async_trait]
impl WeaverTask for SiteMapTask {
    async fn run(&self, config: Arc<WeaverConfig>) -> Result<Option<WritableFile>, BuildError> {
        let target = config.build_dir.clone();
        let sitemap_template = include_str!("../feed_templates/sitemap.xml.liquid");

        let parser = liquid::ParserBuilder::with_stdlib()
            .filter(JSON)
            .build()
            .unwrap();
        let globals = LiquidGlobals::new(
            Arc::new(Mutex::new(Document::default())),
            &all_content_feeds_copy,
            config,
        )
        .await;

        match parser.parse(sitemap_template) {
            Ok(parsed) => match parsed.render(&globals.to_liquid_data()) {
                Ok(result) => Ok(Some(WritableFile {
                    contents: result,
                    path: format!("{}/sitemap.xml", &target).into(),
                    emit: true,
                })),
                Err(err) => {
                    eprintln!("Sitemap template rendering error {:#?}", &err);
                    Err(BuildError::Err(err.to_string()))
                }
            },
            Err(err) => {
                eprintln!("Sitemap template rendering error {:#?}", &err);
                Err(BuildError::Err(err.to_string()))
            }
        }
    }
}
