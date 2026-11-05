mod schema;
mod store;

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub key: String,
}

pub use schema::*;
pub use store::ConfigStore;
