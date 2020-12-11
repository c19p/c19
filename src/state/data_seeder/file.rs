use std::error::Error as StdError;
use crate::state::StateValue;
use crate::state::data_seeder;
use std::fs;
use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

#[derive(Serialize, Deserialize, Debug)]
pub struct File {
    filename: String,
}

#[typetag::serde]
impl data_seeder::DataSeeder for File {
    fn load(&self) -> Result<Box<dyn StateValue>> {
        let data: Box<dyn StateValue> = Box::new(fs::read(&self.filename)?);
        Ok(data)
    }
}
