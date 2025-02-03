use chrono::{DateTime, Datelike, Local, NaiveDate};
use cmaze::algorithms::MazeSpec;
use model::{GameDefinition, SolveResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
};

use crate::{
    helpers::constants::paths::save_data_path,
    settings::{Settings, UpdateCheckInterval},
};

pub mod model {
    use cmaze::{algorithms::MazeType, dims::Dims3D};

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct SolveResult {
        pub moves: i32,
        pub seconds: f32,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash)]
    pub struct GameDefinition {
        pub size: Dims3D,
        pub type_: MazeType,
    }

    impl GameDefinition {
        pub fn from_spec(spec: &MazeSpec) -> Self {
            GameDefinition {
                // player shouldn't be able to play without validating the preset first
                size: spec.size().unwrap(),
                type_: spec.maze_type.unwrap_or_default(),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub last_update_check: Option<DateTime<Local>>,

    #[serde(default)]
    best_results: HashMap<GameDefinition, SolveResult>,

    #[serde(skip_serializing, skip_deserializing)]
    path: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum SaveDataError {
    #[error("Failed to load/save save data file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse save data file: {0}")]
    Serde(#[from] serde_json::Error),
}

impl SaveData {
    pub fn load() -> Result<Self, SaveDataError> {
        match Self::load_from(&save_data_path()) {
            Ok(data) => Ok(data),
            Err(SaveDataError::Io(_)) => Ok(SaveData {
                last_update_check: None,
                best_results: HashMap::new(),
                path: save_data_path(),
            }),
            Err(err) => Err(err),
        }
    }

    pub fn load_or() -> Self {
        Self::load().unwrap_or_else(|_| Self {
            last_update_check: None,
            best_results: HashMap::new(),
            path: save_data_path(),
        })
    }

    fn load_from(path: &Path) -> Result<Self, SaveDataError> {
        Ok(Self {
            path: path.to_owned(),
            ..serde_json::from_reader(File::open(path)?)?
        })
    }

    fn write(&self) -> Result<(), SaveDataError> {
        self.write_to(&self.path)
    }

    fn write_to(&self, path: &Path) -> Result<(), SaveDataError> {
        Ok(serde_json::to_writer(File::create(path)?, self)?)
    }
}

impl SaveData {
    pub fn update_last_check(&mut self) -> Result<(), SaveDataError> {
        self.last_update_check = Some(Local::now());
        self.write()
    }
}

impl SaveData {
    pub fn is_update_checked(&self, settings: &Settings) -> bool {
        use UpdateCheckInterval::*;

        match settings.get_check_interval() {
            Never => true,
            Daily => self.check_date(|d| d),
            Weekly => self.check_date(|d| d.iso_week()),
            Monthly => self.check_date(|d| d.with_day(1).unwrap()),
            Yearly => self.check_date(|d| d.with_day(1).unwrap().with_month(1)),
            Always => false,
        }
    }

    fn check_date<E: Eq>(&self, transform: impl Fn(NaiveDate) -> E) -> bool {
        let today = Local::now().date_naive();
        self.last_update_check
            .map(|lc| lc.date_naive())
            .map(|lc| transform(lc) == transform(today))
            .unwrap_or(false)
    }

    pub fn get_best_result(&self, mode: &MazeSpec) -> Option<(i32, f32)> {
        let result = self
            .best_results
            .get(&GameDefinition::from_spec(mode))
            .copied()?;
        Some((result.moves, result.seconds))
    }

    pub fn set_best_result(
        &mut self,
        mode: &MazeSpec,
        moves: i32,
        seconds: f32,
    ) -> Result<(), SaveDataError> {
        let def = GameDefinition::from_spec(mode);
        let old = self.best_results.get(&def).copied();
        if old.map_or(true, |old| old.seconds > seconds && old.moves >= moves) {
            self.best_results
                .insert(def, SolveResult { moves, seconds });
        }
        self.write()
    }
}
