use std::{fs, sync::Arc};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    BuildError, config::WeaverConfig, renderers::WritableFile, tasks::common::copy_dir_all,
};

use super::WeaverTask;

#[derive(Default)]
pub struct PublicCopyTask;

unsafe impl Send for PublicCopyTask {}
unsafe impl Sync for PublicCopyTask {}

#[async_trait]
impl WeaverTask for PublicCopyTask {
    async fn run(&self, config: Arc<WeaverConfig>) -> Result<Option<WritableFile>, BuildError> {
        let folder_name = config
            .public_dir
            .clone()
            .split('/')
            .next_back()
            .unwrap()
            .to_string();
        let target = format!("{}/{}", config.build_dir.clone(), folder_name);

        if fs::exists(&config.public_dir).expect("failed to check if there was a public directory")
        {
            println!("Copying {} to {}", config.public_dir.clone(), &target);

            copy_dir_all(config.public_dir.clone(), target)
        } else {
            Ok(None)
        }
    }
}
