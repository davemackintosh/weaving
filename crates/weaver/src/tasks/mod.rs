pub mod atom_feed_task;
pub mod clean_build_dir;
pub mod common;
pub mod public_copy_task;
pub mod sitemap_task;
pub mod well_known_copy_task;

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use liquid::model::KString;

use crate::{
    BuildError,
    config::WeaverConfig,
    renderers::{WritableFile, globals::LiquidGlobalsPage},
};

#[async_trait]
pub trait WeaverTask: Send + Sync {
    async fn run(
        &self,
        config: Arc<WeaverConfig>,
        content: &Arc<HashMap<KString, LiquidGlobalsPage>>,
    ) -> Result<Option<WritableFile>, BuildError>;
}
