use serde::{Deserialize, Serialize};
use std::{
    fs::{read_to_string, write},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

pub static STATE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::state_dir().unwrap().join("generic_daw.toml"));

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub last_project: Option<Arc<Path>>,
}

impl State {
    #[must_use]
    pub fn read() -> Option<Self> {
        toml::from_str(&read_to_string(&*STATE_PATH).ok()?).ok()
    }

    pub fn write(&self) {
        write(&*STATE_PATH, toml::to_string(self).unwrap()).unwrap();
    }
}
