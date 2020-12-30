use crate::local_anyhow::Result;
use crate::local_log::debug;
use crate::localstd::{
    fs,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};
use anonify_config::PJ_ROOT_DIR;
use frame_common::crypto::ExportPathSecret;

#[derive(Debug, Clone)]
pub struct StorePathSecrets {
    local_dir_path: PathBuf,
}

impl StorePathSecrets {
    pub fn new<P: AsRef<Path>>(path_secrets_dir: P) -> Self {
        let local_dir_path = (*PJ_ROOT_DIR).to_path_buf().join(path_secrets_dir);
        fs::create_dir_all(&local_dir_path).expect("Failed to create PATH_SECRETS_DIR");
        StorePathSecrets { local_dir_path }
    }

    pub fn save_to_local_filesystem(&self, eps: &ExportPathSecret) -> Result<()> {
        let file_name = hex::encode(&eps.id_as_ref());
        let file_path = self.local_dir_path.join(file_name);
        debug!("Saving a sealed path secret to the path: {:?}", file_path);
        let mut file = fs::File::create(file_path)?;
        serde_json::to_writer(&mut file, &eps)?;
        file.flush()?;
        file.sync_all()?;

        Ok(())
    }

    pub fn load_from_local_filesystem(&self, id: &[u8]) -> Result<ExportPathSecret> {
        let file_name = hex::encode(&id);
        let file_path = self.local_dir_path.join(file_name);
        debug!("Loading a sealed path secret from the path: {:?}", file_path);
        let file = fs::File::open(file_path)?;
        let reader = BufReader::new(file);
        let eps = serde_json::from_reader(reader)?;

        Ok(eps)
    }
}
