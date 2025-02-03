use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use cmaze::{
    algorithms::{
        region_generator::{DepthFirstSearch, RndKruskals}, region_splitter::DefaultRegionSplitter, GeneratorRegistry,
        SplitterRegistry,
    },
    dims::*,
};

use crossterm::event::{read, KeyCode, KeyEvent, KeyEventKind};

use crate::{
    data::SaveData,
    helpers::{constants::paths::settings_path, on_off},
    logging::{self, AppLogger, LoggerOptions, UiLogs},
    renderer::{drawable::Drawable, Cell, Renderer},
    settings::{
        theme::{Theme, ThemeResolver},
        Settings,
    },
    ui,
};

#[cfg(feature = "sound")]
use crate::sound::{track::MusicTrack, SoundPlayer};

#[cfg(feature = "sound")]
use rodio::{self, Source};

use super::{
    activity::{Activities, Activity, ActivityResult, Change},
    event::Event,
    game,
    jobs::Qer,
    Jobs,
};

pub struct App {
    renderer: Renderer,
    activities: Activities,
    data: AppData,
}

pub struct AppData {
    pub settings: Settings,
    pub save: SaveData,
    pub use_data: AppStateData,
    pub screen_size: Dims,
    pub theme: Theme,
    pub logs: UiLogs,
    pub registries: Registries,
    jobs: Jobs,
    app_start: Instant,

    #[cfg(feature = "sound")]
    pub sound_player: SoundPlayer,
    #[cfg(feature = "sound")]
    bgm_track: Option<MusicTrack>,
}

impl AppData {
    pub fn from_start(&self) -> Duration {
        self.app_start.elapsed()
    }

    #[cfg(feature = "sound")]
    pub fn play_bgm(&mut self, track: MusicTrack) {
        if let Some(prev_track) = self.bgm_track {
            if prev_track == track {
                return;
            }
        }

        let volume = if self.settings.get_enable_audio() && self.settings.get_enable_music() {
            self.settings.get_audio_volume() * self.settings.get_music_volume()
        } else {
            0.0
        };
        self.sound_player.set_volume(volume);

        self.bgm_track = Some(track);
        let track = track.get_track().repeat_infinite();
        self.sound_player.play_track(Box::new(track));
    }

    pub fn queuer(&self) -> Qer {
        self.jobs.queuer()
    }
}

pub struct Registries {
    pub region_splitters: SplitterRegistry,
    pub region_generator: GeneratorRegistry,
}

impl App {
    /// Create a new app with a base activity
    ///
    /// This is a convenience method for creating an empty app and pushing
    /// a base activity to it.
    ///
    /// For more information see [`App::empty`] and [`Activities::push`].
    ///
    /// # Arguments
    /// * `base_activity` - The activity to push to the app
    pub fn new(base_activity: Activity, read_only: bool) -> Self {
        let mut s = Self::empty(read_only);
        s.activities.push(base_activity);
        s
    }

    /// Create a new app with no activities
    ///
    /// This method intializes all of the needed components of the app.
    /// - Loads settings,
    /// - loads save data,
    /// - initializes the renderer,
    /// - initializes the sound player (if the feature is enabled),
    /// - initializes the logging system,
    /// - initializes the job queue,
    /// - initializes the registries,
    pub fn empty(read_only: bool) -> Self {
        let renderer = Renderer::new().expect("failed to create renderer");
        let activities = Activities::empty();

        let settings =
            Settings::load_json(settings_path(), read_only).expect("failed to load settings");
        let save = SaveData::load().expect("failed to load save data");
        let use_data = AppStateData::default();
        let jobs = Jobs::new();
        let app_start = Instant::now();
        let frame_size = renderer.frame_size();
        let registries = Registries {
            region_splitters: SplitterRegistry::with_default(
                Arc::new(DefaultRegionSplitter),
                "default",
            ),
            region_generator: {
                let mut reg = GeneratorRegistry::with_default(Arc::new(RndKruskals), "rnd_kruskals");
                reg.register("dfs", Arc::new(DepthFirstSearch));
                reg
            },
        };

        log::info!("Loading theme");
        let resolver = init_theme_resolver();
        let theme_def = settings.get_theme();
        let theme = resolver.resolve(&theme_def);

        let (logger, logs) = AppLogger::new_with_options(
            settings.get_logging_level(),
            LoggerOptions::default()
                .read_only(read_only)
                .file_level(settings.get_file_logging_level()),
        );
        logger.init();

        #[cfg(feature = "sound")]
        let sound_player = SoundPlayer::new(settings.clone());

        Self {
            renderer,
            activities,
            data: AppData {
                app_start,
                settings,
                save,
                use_data,
                screen_size: frame_size,
                jobs,
                theme,
                logs,
                registries,

                #[cfg(feature = "sound")]
                sound_player,
                #[cfg(feature = "sound")]
                bgm_track: None,
            },
        }
    }

    pub fn run(&mut self) -> Option<ActivityResult> {
        log::trace!("Starting main loop");

        let rem_events = 'mainloop: loop {
            while let Some(job) = self.data.jobs.pop() {
                log::trace!("Running job: {:?}", job.name().unwrap_or("<unnamed>"));
                job.call(&mut self.data);
            }

            let mut events = vec![];

            let mut delay = Duration::from_millis(45);
            while let Ok(true) = crossterm::event::poll(delay) {
                let event = read().unwrap();

                self.renderer.on_event(&event);
                self.data.screen_size = self.renderer.frame_size();

                match event {
                    crossterm::event::Event::Key(KeyEvent {
                        code: KeyCode::F(3),
                        kind: KeyEventKind::Press,
                        ..
                    }) => self.switch_debug(),
                    event @ crossterm::event::Event::Mouse(_) => {
                        if self.data.settings.get_enable_mouse() {
                            events.push(Event::Term(event));
                        }
                    }
                    event => events.push(Event::Term(event)),
                }

                // just so we read all events in the frame
                delay = Duration::from_nanos(1)
            }

            while let Some(change) = match self.activities.active_mut() {
                Some(active) => {
                    log::trace!("Updating activity: '{}'", active.name());
                    active
                }
                None => break 'mainloop events,
            }
            .update(std::mem::take(&mut events), &mut self.data)
            {
                match change {
                    Change::Push(activity) => {
                        log::trace!(
                            "Pushed new activity '{}/{}'",
                            activity.source(),
                            activity.name()
                        );
                        self.activities.push(activity);
                    }
                    Change::Pop { n, res } => {
                        self.activities.pop_n(n);
                        events.push(Event::ActiveAfterPop(res));
                        log::trace!("Popped {} activities", n);
                    }
                    Change::PopUntil { name, res } => {
                        self.activities.pop_until(&name);
                        events.push(Event::ActiveAfterPop(res));
                        log::trace!("Popped until '{}'", name);
                    }
                    Change::Replace(activity) => self.activities.replace(activity),
                    Change::ReplaceAt { index, activity } => {
                        self.activities.replace_at(index, activity);
                    }
                }
            }

            self.renderer
                .frame()
                .fill(Cell::styled(' ', self.data.theme.get("background")));

            self.activities
                .active()
                .expect("No active active")
                .screen()
                .draw(self.renderer.frame(), &self.data.theme)
                .unwrap();

            self.data
                .logs
                .draw(Dims(0, 0), self.renderer.frame(), &self.data.theme);

            // TODO: let activities show debug info and about the app itself
            // then we can draw it here

            self.renderer.show().unwrap();
        };

        log::trace!("Main loop ended");

        rem_events.into_iter().find_map(|e| match e {
            Event::ActiveAfterPop(Some(res)) => Some(res),
            _ => None,
        })
    }

    fn switch_debug(&mut self) {
        self.data.use_data.show_debug = !self.data.use_data.show_debug;
        self.data.logs.switch_debug(&self.data.settings);
        log::warn!(
            "Debug mode: {}",
            on_off(self.data.use_data.show_debug, false)
        );
    }

    pub fn activity_count(&self) -> usize {
        self.activities.len()
    }

    pub fn activities(&self) -> &Activities {
        &self.activities
    }

    pub fn activities_mut(&mut self) -> &mut Activities {
        &mut self.activities
    }

    pub fn active_name(&self) -> Option<&str> {
        self.activities.active().map(|a| a.name())
    }

    pub fn data(&self) -> &AppData {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut AppData {
        &mut self.data
    }
}

#[derive(Default)]
pub struct AppStateData {
    pub last_selected_preset: Option<usize>,
    pub show_debug: bool,
}

fn init_theme_resolver() -> ThemeResolver {
    let mut resolver = ThemeResolver::new();

    resolver
        .link("default", "")
        .link("background", "")
        .link("empty", "");

    resolver
        .extend(ui::theme_resolver())
        .extend(game::game_theme_resolver())
        .extend(logging::logging_theme_resolver());

    resolver
}
