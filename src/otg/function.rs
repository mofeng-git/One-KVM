use std::path::Path;

use crate::error::Result;

pub trait GadgetFunction: Send + Sync {
    fn name(&self) -> &str;

    fn endpoints_required(&self) -> u8;

    fn create(&self, gadget_path: &Path) -> Result<()>;

    fn link(&self, config_path: &Path, gadget_path: &Path) -> Result<()>;

    fn unlink(&self, config_path: &Path) -> Result<()>;

    fn cleanup(&self, gadget_path: &Path) -> Result<()>;
}
