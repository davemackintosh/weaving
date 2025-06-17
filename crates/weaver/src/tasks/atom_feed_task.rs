use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use liquid::model::KString;
use tokio::sync::Mutex;

use crate::{
    BuildError,
    config::WeaverConfig,
    document::Document,
    filters::json::JSON,
    renderers::{
        WritableFile,
        globals::{LiquidGlobals, LiquidGlobalsPage},
    },
};

use super::WeaverTask;

#[derive(Default)]
pub struct AtomFeedTask;

unsafe impl Send for AtomFeedTask {}
unsafe impl Sync for AtomFeedTask {}

#[async_trait]
impl WeaverTask for AtomFeedTask {
    async fn run(
        &self,
        config: Arc<WeaverConfig>,
        content: &Arc<HashMap<KString, LiquidGlobalsPage>>,
    ) -> Result<Option<WritableFile>, BuildError> {
        let target = config.build_dir.clone();
        let sitemap_template = include_str!("../templates/atom.xml.liquid");

        let parser = liquid::ParserBuilder::with_stdlib()
            .filter(JSON)
            .build()
            .unwrap();
        let globals =
            LiquidGlobals::new(Arc::new(Mutex::new(Document::default())), content, config).await;

        match parser.parse(sitemap_template) {
            Ok(parsed) => match parsed.render(&globals.to_liquid_data()) {
                Ok(result) => Ok(Some(WritableFile {
                    contents: result,
                    path: format!("{}/atom.xml", &target).into(),
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
