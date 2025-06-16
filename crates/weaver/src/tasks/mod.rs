pub mod common;
pub mod public_copy_task;
pub mod sitemap_task;
pub mod well_known_copy_task;

use std::sync::Arc;

use async_trait::async_trait;

use crate::{BuildError, config::WeaverConfig, renderers::WritableFile};

#[async_trait]
pub trait WeaverTask: Send + Sync {
    async fn run(&self, config: Arc<WeaverConfig>) -> Result<Option<WritableFile>, BuildError>;
}
