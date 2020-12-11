pub mod file;

use std::error::Error as StdError;
use crate::state::StateValue;

#[typetag::serde(tag = "kind")]
pub trait DataSeeder: std::fmt::Debug + Send + Sync {
    fn load(&self) -> Result<Box<dyn StateValue>, Box<dyn StdError>>;
}

