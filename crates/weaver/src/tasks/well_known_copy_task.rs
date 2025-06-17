use std::{collections::HashMap, fs, sync::Arc};

use async_trait::async_trait;
use liquid::model::KString;

use crate::{
    BuildError,
    config::WeaverConfig,
    renderers::{WritableFile, globals::LiquidGlobalsPage},
    tasks::common::copy_dir_all,
};

use super::WeaverTask;

#[derive(Default)]
pub struct WellKnownCopyTask;

unsafe impl Send for WellKnownCopyTask {}
unsafe impl Sync for WellKnownCopyTask {}

#[async_trait]
impl WeaverTask for WellKnownCopyTask {
    async fn run(
        &self,
        config: Arc<WeaverConfig>,
        _content: &Arc<HashMap<KString, LiquidGlobalsPage>>,
    ) -> Result<Option<WritableFile>, BuildError> {
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
