pub mod common;
pub mod public_copy_task;
pub mod well_known_copy_task;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{BuildError, Weaver, renderers::WritableFile};

#[async_trait]
pub trait WeaverTask: Send + Sync + Default {
    async fn run(
        &mut self,
        weaver_instance: Arc<Mutex<&Weaver>>,
    ) -> Result<Option<WritableFile>, BuildError>;
}
