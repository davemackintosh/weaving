use std::{fs, path::Path, sync::Arc};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{BuildError, Weaver, renderers::WritableFile, tasks::common::copy_dir_all};

use super::WeaverTask;

#[derive(Default)]
pub struct WellKnownCopyTask;

unsafe impl Send for WellKnownCopyTask {}
unsafe impl Sync for WellKnownCopyTask {}

#[async_trait]
impl WeaverTask for WellKnownCopyTask {
    async fn run(
        &mut self,
        weaver_instance: Arc<Mutex<&Weaver>>,
    ) -> Result<Option<WritableFile>, BuildError> {
        let weaver = weaver_instance.lock().await;

        let config = Arc::clone(&weaver.config);
        let well_known_path = format!("{}/.well-known", &config.base_dir);
        let target = format!("{}/.well-known", config.build_dir.clone());

        if fs::exists(well_known_path).expect("failed to check if there was a public directory") {
            println!("Copying {} to {}", config.public_dir.clone(), &target);

            copy_dir_all(config.public_dir.clone(), target)
        } else {
            Ok(None)
        }
    }
}
