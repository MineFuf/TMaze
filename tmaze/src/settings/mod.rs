mod attribute;
pub mod theme;

use cmaze::{
    algorithms::{MazeSpec, MazeSpecType},
    dims::{Dims, Offset},
};
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use theme::ThemeDefinition;

use crate::{
    app::{self, app::AppData, Activity, ActivityHandler, Change},
    helpers::constants::paths::settings_path,
    menu_actions,
    renderer::MouseGuard,
    ui::{split_menu_actions, Menu, MenuAction, MenuConfig, MenuItem, OptionDef, Popup, Screen},
};

#[cfg(feature = "sound")]
use crate::sound::create_audio_settings;

const DEFAULT_SETTINGS_JSON: &str = include_str!("./default_settings.json5");

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum CameraMode {
    #[default]
    CloseFollow,
    EdgeFollow {
        x: Offset,
        y: Offset,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MazePreset {
    pub title: String,
    pub description: Option<String>,

    #[serde(default)]
    pub default: bool,

    // TODO: make `serde(flatten)` once switched to TOML/JSON
    #[serde(flatten)]
    pub maze_spec: MazeSpec,
}

impl MazePreset {
    pub fn short_desc(&self) -> String {
        let (size, cells): (_, usize) = match &self.maze_spec.inner_spec {
            MazeSpecType::Regions { regions, .. } => (
                self.maze_spec.size().unwrap(),
                regions.iter().map(|r| r.mask.enabled_count()).sum(),
            ),
            MazeSpecType::Simple { mask, .. } => {
                let size = self.maze_spec.size().unwrap();
                (
                    size,
                    mask.as_ref()
                        .map(|m| m.enabled_count())
                        .unwrap_or(size.product() as usize),
                )
            }
        };

        if size.2 == 1 {
            format!("{}: {}x{} ({} cells)", self.title, size.0, size.1, cells)
        } else {
            format!(
                "{}: {}x{}x{} ({} cells)",
                self.title, size.0, size.1, size.2, cells
            )
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum UpdateCheckInterval {
    Never,
    #[default]
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Always,
}

#[derive(Debug, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[serde(rename = "Settings")]
// FIXME: separate sections into their own struct
pub struct SettingsInner {
    // general
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub logging_level: Option<String>,
    #[serde(default)]
    pub debug_logging_level: Option<String>,
    #[serde(default)]
    pub file_logging_level: Option<String>,

    // viewport
    #[serde(default)]
    pub slow: Option<bool>,
    #[serde(default)]
    pub disable_tower_auto_up: Option<bool>,
    #[serde(default)]
    pub camera_mode: Option<CameraMode>,
    #[serde(default)]
    pub camera_smoothing: Option<f32>,
    #[serde[default]]
    pub player_smoothing: Option<f32>,
    #[serde(default)]
    pub viewport_margin: Option<(i32, i32)>,

    // navigation
    #[serde(default)]
    pub enable_mouse: Option<bool>,
    #[serde(default)]
    pub enable_dpad: Option<bool>,
    #[serde(default)]
    pub landscape_dpad_on_left: Option<bool>,
    #[serde(default)]
    pub dpad_swap_up_down: Option<bool>,
    #[serde(default)]
    pub enable_margin_around_dpad: Option<bool>,
    #[serde(default)]
    pub enable_dpad_highlight: Option<bool>,

    // update check
    #[serde(default)]
    pub update_check_interval: Option<UpdateCheckInterval>,
    #[serde(default)]
    pub display_update_check_errors: Option<bool>,

    // audio
    #[serde(default)]
    pub enable_audio: Option<bool>,
    #[serde(default)]
    pub audio_volume: Option<f32>,
    #[serde(default)]
    pub enable_music: Option<bool>,
    #[serde(default)]
    pub music_volume: Option<f32>,

    // presets
    #[serde(default)]
    pub presets: Option<Vec<MazePreset>>,
    // TODO: it's not possible in RON to have a HashMap with flattened keys,
    // so we will support it in different way formats
    // once we support them - this would mean dropping RON support
    // https://github.com/ron-rs/ron/issues/115
    // pub unknown_fields: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct Settings {
    shared: Arc<RwLock<SettingsInner>>,
    path: PathBuf,
    read_only: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let settings = SettingsInner::default();
        Self {
            shared: Arc::new(RwLock::new(settings)),
            path: settings_path(),
            read_only: false,
        }
    }
}

#[allow(dead_code)]
impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn is_ro(&self) -> bool {
        self.read_only
    }

    pub fn read(&self) -> std::sync::RwLockReadGuard<SettingsInner> {
        self.shared.read().unwrap()
    }

    pub fn write(&mut self) -> std::sync::RwLockWriteGuard<SettingsInner> {
        self.shared.write().unwrap()
    }
}

impl Settings {
    pub fn get_theme(&self) -> ThemeDefinition {
        let theme_name = self.read().theme.clone();
        if let Some(theme_name) = theme_name {
            ThemeDefinition::load_by_name(&theme_name).expect("could not load the theme")
        } else {
            ThemeDefinition::load_default(self.read_only).expect("could not load the default theme")
        }
    }

    pub fn get_logging_level(&self) -> log::Level {
        self.read()
            .logging_level
            .clone()
            .and_then(|level| level.parse().ok())
            .unwrap_or(log::Level::Info)
    }

    pub fn get_debug_logging_level(&self) -> log::Level {
        self.read()
            .debug_logging_level
            .clone()
            .and_then(|level| level.parse().ok())
            .unwrap_or(log::Level::Info)
    }

    pub fn get_file_logging_level(&self) -> log::Level {
        self.read()
            .file_logging_level
            .clone()
            .and_then(|level| level.parse().ok())
            .unwrap_or(log::Level::Info)
    }

    pub fn get_slow(&self) -> bool {
        self.read().slow.unwrap_or_default()
    }

    pub fn set_slow(&mut self, value: bool) -> &mut Self {
        self.write().slow = Some(value);
        self
    }

    pub fn get_disable_tower_auto_up(&self) -> bool {
        self.read().disable_tower_auto_up.unwrap_or_default()
    }

    pub fn set_disable_tower_auto_up(&mut self, value: bool) -> &mut Self {
        self.write().disable_tower_auto_up = Some(value);
        self
    }

    pub fn get_camera_mode(&self) -> CameraMode {
        self.read().camera_mode.unwrap_or_default()
    }

    pub fn set_camera_mode(&mut self, value: CameraMode) -> &mut Self {
        self.write().camera_mode = Some(value);
        self
    }

    pub fn get_camera_smoothing(&self) -> f32 {
        self.read().camera_smoothing.unwrap_or(0.5).clamp(0.5, 1.0)
    }

    pub fn set_camera_smoothing(&mut self, value: f32) -> &mut Self {
        self.write().camera_smoothing = Some(value.clamp(0.5, 1.0));
        self
    }

    pub fn get_player_smoothing(&self) -> f32 {
        self.read().player_smoothing.unwrap_or(0.8).clamp(0.5, 1.0)
    }

    pub fn set_player_smoothing(&mut self, value: f32) -> &mut Self {
        self.write().player_smoothing = Some(value.clamp(0.5, 1.0));
        self
    }

    pub fn get_viewport_margin(&self) -> Dims {
        self.read()
            .viewport_margin
            .map(Dims::from)
            .unwrap_or(Dims(4, 3))
    }

    pub fn set_viewport_margin(&mut self, value: Dims) -> &mut Self {
        self.write().viewport_margin = Some(value.into());
        self
    }

    pub fn get_enable_mouse(&self) -> bool {
        self.read().enable_mouse.unwrap_or(true)
    }

    pub fn set_enable_mouse(&mut self, value: bool) -> &mut Self {
        self.write().enable_mouse = Some(value);
        self
    }

    pub fn get_enable_dpad(&self) -> bool {
        self.read().enable_dpad.unwrap_or(false)
    }

    pub fn set_enable_dpad(&mut self, value: bool) -> &mut Self {
        self.write().enable_dpad = Some(value);
        self
    }

    pub fn get_landscape_dpad_on_left(&self) -> bool {
        self.read().landscape_dpad_on_left.unwrap_or(false)
    }

    pub fn set_landscape_dpad_on_left(&mut self, value: bool) -> &mut Self {
        self.write().landscape_dpad_on_left = Some(value);
        self
    }

    pub fn get_dpad_swap_up_down(&self) -> bool {
        self.read().dpad_swap_up_down.unwrap_or(false)
    }

    pub fn set_dpad_swap_up_down(&mut self, value: bool) -> &mut Self {
        self.write().dpad_swap_up_down = Some(value);
        self
    }

    pub fn get_enable_margin_around_dpad(&self) -> bool {
        self.read().enable_margin_around_dpad.unwrap_or(false)
    }

    pub fn set_enable_margin_around_dpad(&mut self, value: bool) -> &mut Self {
        self.write().enable_margin_around_dpad = Some(value);
        self
    }

    pub fn get_enable_dpad_highlight(&self) -> bool {
        self.read().enable_dpad_highlight.unwrap_or(true)
    }

    pub fn set_enable_dpad_highlight(&mut self, value: bool) -> &mut Self {
        self.write().enable_dpad_highlight = Some(value);
        self
    }

    pub fn set_check_interval(&mut self, value: UpdateCheckInterval) -> &mut Self {
        self.write().update_check_interval = Some(value);
        self
    }

    pub fn get_check_interval(&self) -> UpdateCheckInterval {
        self.read().update_check_interval.unwrap_or_default()
    }

    pub fn get_display_update_check_errors(&self) -> bool {
        self.read().display_update_check_errors.unwrap_or(true)
    }

    pub fn set_display_update_check_errors(&mut self, value: bool) -> &mut Self {
        self.write().display_update_check_errors = Some(value);
        self
    }

    pub fn get_enable_audio(&self) -> bool {
        self.read().enable_audio.unwrap_or_default()
    }

    pub fn set_enable_audio(&mut self, value: bool) -> &mut Self {
        self.write().enable_audio = Some(value);
        self
    }

    pub fn get_audio_volume(&self) -> f32 {
        self.read().audio_volume.unwrap_or_default().clamp(0., 1.)
    }

    pub fn set_audio_volume(&mut self, value: f32) -> &mut Self {
        self.write().audio_volume = Some(value.clamp(0., 1.));
        self
    }

    pub fn get_enable_music(&self) -> bool {
        self.read().enable_music.unwrap_or_default()
    }

    pub fn set_enable_music(&mut self, value: bool) -> &mut Self {
        self.write().enable_music = Some(value);
        self
    }

    pub fn get_music_volume(&self) -> f32 {
        self.read().music_volume.unwrap_or_default().clamp(0., 1.)
    }

    pub fn set_music_volume(&mut self, value: f32) -> &mut Self {
        self.write().music_volume = Some(value.clamp(0., 1.));
        self
    }

    pub fn set_presets(&mut self, value: Vec<MazePreset>) -> &mut Self {
        self.write().presets = Some(value);
        self
    }

    pub fn get_presets(&self) -> Vec<MazePreset> {
        self.read().presets.clone().unwrap_or_default()
    }
}

// JSON
impl Settings {
    pub fn load_json(path: PathBuf, read_only: bool) -> io::Result<Self> {
        let settings_string = fs::read_to_string(&path);
        let settings: SettingsInner = if let Ok(settings_string) = settings_string {
            json5::from_str(&settings_string)
                .expect("Could not parse settings file: check the syntax")
        } else {
            if !read_only {
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(&path, DEFAULT_SETTINGS_JSON)?;
            }
            json5::from_str(DEFAULT_SETTINGS_JSON).unwrap()
        };

        Ok(Self {
            shared: Arc::new(RwLock::new(settings)),
            path,
            read_only,
        })
    }

    pub fn reset_json(&mut self) {
        *self.write() = json5::from_str(DEFAULT_SETTINGS_JSON).unwrap();

        let path = settings_path();
        fs::write(&path, DEFAULT_SETTINGS_JSON).unwrap();

        self.path = path;
    }

    pub fn reset_json_config(path: PathBuf) {
        fs::write(path, DEFAULT_SETTINGS_JSON).unwrap();
    }
}

struct OtherSettingsPopup(Popup, MouseGuard);

impl OtherSettingsPopup {
    fn new(settings: &Settings) -> Self {
        let popup = Popup::new(
            "Other settings".to_string(),
            vec![
                "Path to the current settings:".to_string(),
                format!(" {}", settings.path().to_string_lossy().to_string()),
                "".to_string(),
                "Other settings are not implemented in UI yet.".to_string(),
                "Please edit the settings file directly.".to_string(),
            ],
        );

        Self(popup, MouseGuard::new().unwrap())
    }
}

impl ActivityHandler for OtherSettingsPopup {
    fn update(&mut self, events: Vec<app::Event>, data: &mut AppData) -> Option<Change> {
        self.0.update(events, data)
    }

    fn screen(&self) -> &dyn Screen {
        &self.0
    }
}

pub struct SettingsActivity {
    actions: Vec<MenuAction<Change>>,
    menu: Menu,
}

impl SettingsActivity {
    fn other_settings_popup(settings: &Settings) -> Activity {
        Activity::new_base_boxed("settings".to_string(), OtherSettingsPopup::new(settings))
    }
}

#[allow(clippy::new_without_default)]
impl SettingsActivity {
    pub fn new() -> Self {
        let options = menu_actions!(
            "Audio" on "sound" -> data => Change::push(create_audio_settings(data)),
            "Controls" -> data => Change::push(create_controls_settings(data)),
            "Other settings" -> data => Change::push(SettingsActivity::other_settings_popup(&data.settings)),
            "Back" -> _ => Change::pop_top(),
        );

        let (options, actions) = split_menu_actions(options);

        let menu_config = MenuConfig::new("Settings", options).subtitle("Changes are not saved");

        Self {
            actions,
            menu: Menu::new(menu_config),
        }
    }

    pub fn new_activity() -> Activity {
        Activity::new_base_boxed("settings".to_string(), Self::new())
    }
}

impl ActivityHandler for SettingsActivity {
    fn update(&mut self, events: Vec<app::Event>, data: &mut AppData) -> Option<Change> {
        match self.menu.update(events, data)? {
            Change::Pop {
                res: Some(sub_activity),
                ..
            } => {
                let index = *sub_activity
                    .downcast::<usize>()
                    .expect("menu should return index");
                Some((self.actions[index])(data))
            }
            res => Some(res),
        }
    }

    fn screen(&self) -> &dyn Screen {
        &self.menu
    }
}

pub fn create_controls_settings(data: &mut AppData) -> Activity {
    let menu_config = MenuConfig::new(
        "Controls settings",
        [
            MenuItem::Option(OptionDef {
                text: "Enable mouse input".into(),
                val: data.settings.get_enable_mouse(),
                fun: Box::new(|enabled, data| {
                    *enabled = !*enabled;
                    data.settings.set_enable_mouse(*enabled);
                }),
            }),
            MenuItem::Option(OptionDef {
                text: "Enable dpad".into(),
                val: data.settings.get_enable_dpad(),
                fun: Box::new(|enabled, data| {
                    *enabled = !*enabled;
                    data.settings.set_enable_dpad(*enabled);
                }),
            }),
            MenuItem::Option(OptionDef {
                text: "Left-handed dpad".into(),
                val: data.settings.get_landscape_dpad_on_left(),
                fun: Box::new(|is_on_left, data| {
                    *is_on_left = !*is_on_left;
                    data.settings.set_landscape_dpad_on_left(*is_on_left);
                }),
            }),
            MenuItem::Option(OptionDef {
                text: "Swap Up and Down buttons".into(),
                val: data.settings.get_dpad_swap_up_down(),
                fun: Box::new(|do_swap, data| {
                    *do_swap = !*do_swap;
                    data.settings.set_dpad_swap_up_down(*do_swap);
                }),
            }),
            MenuItem::Option(OptionDef {
                text: "Enable margin around dpad".into(),
                val: data.settings.get_enable_margin_around_dpad(),
                fun: Box::new(|enabled, data| {
                    *enabled = !*enabled;
                    data.settings.set_enable_margin_around_dpad(*enabled);
                }),
            }),
            MenuItem::Option(OptionDef {
                text: "Enable dpad highlight".into(),
                val: data.settings.get_enable_dpad_highlight(),
                fun: Box::new(|enabled, data| {
                    *enabled = !*enabled;
                    data.settings.set_enable_dpad_highlight(*enabled);
                }),
            }),
            MenuItem::Separator,
            MenuItem::Text("Exit".into()),
        ],
    );

    Activity::new_base_boxed("controls settings", Menu::new(menu_config))
}
