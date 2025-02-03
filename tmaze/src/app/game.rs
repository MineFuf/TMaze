use std::mem;

use cmaze::{
    algorithms::GeneratorError,
    array::Array2DView,
    dims::*,
    game::{GameProperities, RunningGame, RunningGameState, RunningJob},
    gameboard::{Cell, CellWall},
    progress::Progress,
};

use crate::{
    app::{game_state::GameData, GameViewMode},
    helpers::{
        constants, is_release, maze2screen, maze2screen_3d, maze_render_size, strings, LineDir,
    },
    lerp, menu_actions,
    renderer::{self, Frame},
    settings::{
        self,
        theme::{Theme, ThemeResolver},
        CameraMode, MazePreset, Settings, SettingsActivity,
    },
    ui::{
        self,
        helpers::format_duration,
        multisize_duration_format, split_menu_actions,
        usecase::dpad::{DPad, DPadType},
        Menu, MenuAction, MenuConfig, Popup, ProgressBar, Rect, Screen,
    },
};

#[cfg(feature = "sound")]
#[allow(unused_imports)]
use crate::sound::{track::MusicTrack, SoundPlayer};

use crossterm::event::{Event as TermEvent, KeyCode, KeyEvent};

#[cfg(feature = "sound")]
#[allow(unused_imports)]
use rodio::Source;

use super::{
    app::{AppData, AppStateData, Registries},
    Activity, ActivityHandler, Change, Event,
};

pub fn create_controls_popup() -> Activity {
    let popup = Popup::new(
        "Controls".to_string(),
        [
            "~ In game",
            " WASD and arrows: move",
            " Space: switch adventure/spectaror mode",
            " Q, F or L: move down",
            " E, R or P: move up",
            " With SHIFT move at the end in single dir",
            " Escape: pause menu",
            "",
            "~ In end game popup",
            " Enter or space: main menu",
            " Q: quit TMaze",
            " R: restart game",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>(),
    );

    Activity::new_base_boxed("controls".to_string(), popup)
}

pub struct MainMenu {
    menu: Menu,
    actions: Vec<MenuAction<Change>>,
}

#[allow(clippy::new_without_default)]
impl MainMenu {
    pub fn new() -> Self {
        let options = menu_actions!(
            "New Game" -> data => Self::start_new_game(&data.settings, &data.use_data),
            "Settings" -> _ => Self::show_settings_screen(),
            "Controls" -> _ => Self::show_controls_popup(),
            "About" -> _ => Self::show_about_popup(),
            "Quit" -> _ => Change::pop_top(),
        );

        let (options, actions) = split_menu_actions(options);

        Self {
            menu: Menu::new(MenuConfig::new("TMaze", options).counted()),
            actions,
        }
    }

    fn show_settings_screen() -> Change {
        Change::push(Activity::new_base_boxed(
            "settings".to_string(),
            settings::SettingsActivity::new(),
        ))
    }

    fn show_controls_popup() -> Change {
        Change::push(create_controls_popup())
    }

    fn show_about_popup() -> Change {
        const FEATURE_LIST: [(&str, bool); 2] = [
            ("updates", cfg!(feature = "updates")),
            ("sound", cfg!(feature = "sound")),
        ];

        let mut lines = vec![
            "This is simple maze solving game".to_string(),
            "Supported algorithms:".to_string(),
            "    - Depth-first search".to_string(),
            "    - Kruskal's algorithm".to_string(),
            "Supports 3D mazes".to_string(),
            "".to_string(),
            "Created by:".to_string(),
            format!("    - {}", env!("CARGO_PKG_AUTHORS")),
            "".to_string(),
            "Version:".to_string(),
            format!("    {}", env!("CARGO_PKG_VERSION")),
        ];

        {
            let (enabled, disabled) = FEATURE_LIST
                .into_iter()
                .partition::<Vec<_>, _>(|(_, enabled)| *enabled);

            let enabled = enabled
                .into_iter()
                .map(|(name, _)| format!("    - {}", name))
                .collect::<Vec<_>>();

            let disabled = disabled
                .into_iter()
                .map(|(name, _)| format!("    - {}", name))
                .collect::<Vec<_>>();

            if !enabled.is_empty() {
                lines.push("".to_string());
                lines.push("Enabled features:".to_string());
                lines.extend(enabled);
            }

            if !disabled.is_empty() {
                lines.push("".to_string());
                lines.push("Disabled features:".to_string());
                lines.extend(disabled);
            }
        }

        let popup = Popup::new("About".to_string(), lines);

        Change::push(Activity::new_base_boxed("about".to_string(), popup))
    }

    fn start_new_game(settings: &Settings, use_data: &AppStateData) -> Change {
        Change::push(Activity::new_base_boxed(
            "maze size",
            MazePresetMenu::new(settings, use_data),
        ))
    }

    #[cfg(feature = "sound")]
    fn play_menu_bgm(data: &mut AppData) {
        data.play_bgm(MusicTrack::Menu);
    }
}

impl ActivityHandler for MainMenu {
    fn update(&mut self, events: Vec<super::Event>, data: &mut AppData) -> Option<Change> {
        #[cfg(feature = "sound")]
        Self::play_menu_bgm(data);

        match self.menu.update(events, data)? {
            Change::Pop {
                res: Some(sub_activity),
                ..
            } => {
                let index = *sub_activity
                    .downcast::<usize>()
                    .expect("menu should return index");
                Some(self.actions[index](data))
            }
            res => Some(res),
        }
    }

    fn screen(&self) -> &dyn ui::Screen {
        &self.menu
    }
}

pub struct MazePresetMenu {
    menu: Menu,
    presets: Vec<MazePreset>,
}

impl MazePresetMenu {
    pub fn new(settings: &Settings, app_state_data: &AppStateData) -> Self {
        let mut menu_config = MenuConfig::new_from_strings(
            "Maze size".to_string(),
            settings
                .get_presets()
                .iter()
                .map(|maze| maze.title.clone())
                .collect::<Vec<_>>(),
        );

        let default = app_state_data
            .last_selected_preset
            .or_else(|| settings.get_presets().iter().position(|maze| maze.default));

        if let Some(i) = default {
            menu_config = menu_config.default(i);
        }

        let menu = Menu::new(menu_config);

        let presets = settings.get_presets().to_vec();

        Self { menu, presets }
    }
}

impl ActivityHandler for MazePresetMenu {
    fn update(&mut self, events: Vec<super::Event>, data: &mut AppData) -> Option<Change> {
        match self.menu.update(events, data) {
            Some(change) => match change {
                Change::Pop {
                    res: Some(size), ..
                } => {
                    let index = *size.downcast::<usize>().expect("menu should return index");
                    data.use_data.last_selected_preset = Some(index);

                    let preset = self.presets[index].clone();

                    Some(Change::push(Activity::new_base_boxed(
                        "maze_gen".to_string(),
                        MazeGenerationActivity::new(preset, &data.registries),
                    )))
                }
                res => Some(res),
            },
            None => None,
        }
    }

    fn screen(&self) -> &dyn ui::Screen {
        &self.menu
    }
}

pub struct MazeGenerationActivity {
    comm: Result<RunningJob<Option<RunningGame>>, GeneratorError>,
    preset: MazePreset,
    progress_bar: ProgressBar,
}

impl MazeGenerationActivity {
    pub fn new(preset: MazePreset, registries: &Registries) -> Self {
        let text = preset.short_desc();
        let game_props = GameProperities {
            maze_spec: preset.maze_spec.clone(),
        };

        let progress_bar = ProgressBar::new(format!("Generating maze: {}", text));

        Self {
            comm: RunningGame::prepare(
                game_props,
                &registries.region_generator,
                &registries.region_splitters,
            ),
            preset,
            progress_bar,
        }
    }
}

impl ActivityHandler for MazeGenerationActivity {
    fn update(&mut self, events: Vec<super::Event>, data: &mut AppData) -> Option<Change> {
        for event in events {
            #[allow(clippy::collapsible_match)]
            match event {
                Event::Term(TermEvent::Key(KeyEvent { code, kind, .. })) if !is_release(kind) => {
                    match code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            let mut comm = Err(GeneratorError::Unknown); // dummy value
                            mem::swap(&mut self.comm, &mut comm);
                            if let Ok(comm) = comm {
                                comm.progress.stop();
                                let _ = comm.handle.join().unwrap();
                            };
                            return Some(Change::pop(2));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        match self.comm {
            Ok(ref comm) if comm.handle.is_finished() => {
                let mut comm = Err(GeneratorError::Unknown); // dummy value
                mem::swap(&mut self.comm, &mut comm);
                let res = comm
                    .ok()
                    .unwrap()
                    .handle
                    .join()
                    .expect("Could not join maze generation thread");

                match res {
                    Some(game) => {
                        let game_data = GameData {
                            camera_pos: maze2screen_3d(game.get_player_pos()),
                            game,
                            view_mode: GameViewMode::Adventure,
                            player_char: constants::get_random_player_char(),
                            maze_preset: self.preset.clone(),
                        };
                        Some(Change::replace(Activity::new_base_boxed(
                            "game".to_string(),
                            GameActivity::new(game_data, data),
                        )))
                    }
                    None => {
                        const MSG: &str = "Maze generation was aborted";
                        log::info!("{}", MSG);

                        Some(Change::pop_top())
                    }
                }
            }

            Ok(ref comm) => {
                let progress @ Progress { done, from, .. } = comm.progress();
                self.progress_bar.update_progress(progress.percent() as f64);
                self.progress_bar.update_title(format!(
                    "Generating maze: {}/{} - {:.2} %",
                    done,
                    from,
                    done as f64 / from as f64 * 100.0
                ));
                None
            }

            Err(ref err) => {
                const UNKNOWN_MSG: &str = "Unknown error while generating maze";
                const VALIDATION_MSG: &str = "Invalid maze preset, please check it";
                let msg = match err {
                    GeneratorError::Unknown => UNKNOWN_MSG,
                    GeneratorError::Validation => VALIDATION_MSG,
                };
                log::error!("{}: {:?}", msg, err);

                Some(Change::replace(Activity::new_base_boxed(
                    "game gen error",
                    Popup::new("Error".to_string(), vec![msg.to_string()]),
                )))
            }
        }
    }

    fn screen(&self) -> &dyn ui::Screen {
        &self.progress_bar
    }
}

pub struct PauseMenu {
    menu: Menu,
    actions: Vec<MenuAction<Change>>,
}

#[allow(clippy::new_without_default)]
impl PauseMenu {
    pub fn new() -> Self {
        let options = menu_actions!(
            "Resume" -> _ => Change::pop_top(),
            "Main Menu" -> _ => Change::pop_until("main menu"),
            "Controls" -> _ => Change::push(create_controls_popup()),
            "Settings" -> _ => Change::push(SettingsActivity::new_activity()),
            "Quit" -> _ => Change::pop_all(),
        );

        let (options, actions) = split_menu_actions(options);

        let menu = Menu::new(MenuConfig::new("Paused", options));

        Self { menu, actions }
    }
}

impl ActivityHandler for PauseMenu {
    fn update(&mut self, events: Vec<Event>, data: &mut AppData) -> Option<Change> {
        match self.menu.update(events, data) {
            Some(change) => match change {
                Change::Pop { res: Some(res), .. } => {
                    let index = *res.downcast::<usize>().expect("menu should return index");

                    Some((self.actions[index])(data))
                }
                res => Some(res),
            },
            None => None,
        }
    }

    fn screen(&self) -> &dyn Screen {
        &self.menu
    }
}

pub struct EndGamePopup {
    popup: Popup,
    preset: MazePreset,
}

impl EndGamePopup {
    pub fn new(game: &RunningGame, preset: MazePreset) -> Self {
        let maze_size = game.get_maze().size();
        let texts = vec![
            format!("Time:  {}", format_duration(game.get_elapsed().unwrap())),
            format!("Moves: {}", game.get_move_count()),
            format!("Size:  {}x{}x{}", maze_size.0, maze_size.1, maze_size.2,),
        ];

        let popup = Popup::new("You won".to_string(), texts);

        Self { popup, preset }
    }
}

impl ActivityHandler for EndGamePopup {
    fn update(&mut self, events: Vec<Event>, data: &mut AppData) -> Option<Change> {
        match self.popup.update(events, data) {
            Some(Change::Pop {
                n: 1,
                res: Some(code),
            }) => match code.downcast::<KeyCode>() {
                Ok(b) => match *b {
                    KeyCode::Char('r') => Some(Change::replace(Activity::new_base_boxed(
                        "game",
                        MazeGenerationActivity::new(self.preset.clone(), &data.registries),
                    ))),
                    KeyCode::Char('q') => Some(Change::pop_all()),
                    KeyCode::Enter | KeyCode::Char(' ') => Some(Change::pop_top()),
                    _ => None,
                },
                _ => panic!("expected `KeyCode` from `Popup`"),
            },
            res => res,
        }
    }

    fn screen(&self) -> &dyn Screen {
        &self.popup
    }
}

pub struct GameActivity {
    camera_mode: CameraMode,
    data: GameData,
    maze_board: MazeBoard,
    show_debug: bool,

    // spacing
    margins: Dims,
    viewport_rect: Rect,
    dpad_rect: Option<Rect>,

    // smooth
    sm_camera_pos: Dims3D,
    sm_player_pos: Dims3D,

    // touch
    touch_controls: Option<Box<DPad>>,
}

impl GameActivity {
    pub fn new(game: GameData, app_data: &mut AppData) -> Self {
        let settings = &app_data.settings;

        let camera_mode = settings.get_camera_mode();
        let maze_board = MazeBoard::new(&game.game, &app_data.theme);
        let margins = settings.get_viewport_margin();

        #[cfg(feature = "sound")]
        app_data.play_bgm(MusicTrack::choose_for_maze(game.game.get_maze()));

        let sm_camera_pos = game.camera_pos;
        let sm_player_pos = maze2screen_3d(game.game.get_player_pos());

        Self {
            camera_mode,
            data: game,
            maze_board,
            show_debug: false,

            margins,
            viewport_rect: Rect::sized(app_data.screen_size),
            dpad_rect: None,

            sm_camera_pos,
            sm_player_pos,

            touch_controls: None,
        }
    }

    /// Returns the size of the viewport and whether the floor fits in the viewport
    pub fn viewport_size(&self, screen_size: Dims) -> (Dims, bool) {
        let vp_size = screen_size - self.margins * 2;

        let maze_frame = &self.maze_board.frames[self.data.game.get_player_pos().2 as usize];
        let floor_size = maze_frame.size;

        let does_fit = floor_size.0 <= vp_size.0 && floor_size.1 <= vp_size.1;

        (if does_fit { floor_size } else { vp_size }, does_fit)
    }

    fn current_floor_frame(&self) -> &Frame {
        &self.maze_board.frames[self.data.camera_pos.2 as usize]
    }

    fn render_meta_texts(&self, frame: &mut Frame, theme: &Theme, vp: Rect) {
        let max_width = (vp.size().0 / 2 + 1) as usize;

        let pl_pos = self.data.game.get_player_pos() + Dims3D(1, 1, 1);

        // texts
        let from_start =
            multisize_duration_format(self.data.game.get_elapsed().unwrap(), max_width);
        let move_count = strings::multisize_string(
            [
                format!("{} moves", self.data.game.get_move_count()),
                format!("{}m", self.data.game.get_move_count()),
            ],
            max_width,
        );

        let pos_text = if self.data.game.get_maze().size().2 > 1 {
            strings::multisize_string(
                [
                    format!("x:{} y:{} floor:{}", pl_pos.0, pl_pos.1, pl_pos.2),
                    format!("x:{} y:{} f:{}", pl_pos.0, pl_pos.1, pl_pos.2),
                    format!("{}:{}:{}", pl_pos.0, pl_pos.1, pl_pos.2),
                ],
                max_width,
            )
        } else {
            strings::multisize_string(
                [
                    format!("x:{} y:{}", pl_pos.0, pl_pos.1),
                    format!("x:{} y:{}", pl_pos.0, pl_pos.1),
                    format!("{}:{}", pl_pos.0, pl_pos.1),
                ],
                max_width,
            )
        };

        let view_mode = self.data.view_mode;
        let view_mode = strings::multisize_string(view_mode.to_multisize_strings(), max_width);

        let tl = vp.start - Dims(0, 1);
        let br = vp.start + vp.size();

        let style = theme["text"];
        let mut draw = |text: &str, pos| frame.draw(pos, text, style);

        draw(&pos_text, tl);
        draw(view_mode, Dims(br.0 - view_mode.len() as i32, tl.1));
        draw(&move_count, Dims(tl.0, br.1));
        draw(&from_start, Dims(br.0 - from_start.len() as i32, br.1));
    }

    pub fn render_visited_places(&self, frame: &mut Frame, maze_pos: Dims, theme: &Theme) {
        use CellWall::{Down, Up};

        let game = &self.data.game;
        for (move_pos, _) in game.get_moves() {
            let cell = game.get_maze().board.get_cell(*move_pos).unwrap();
            if move_pos.2 == game.get_player_pos().2 && cell.get_wall(Up) && cell.get_wall(Down) {
                let real_pos = maze2screen(*move_pos) + maze_pos;
                frame.draw(real_pos, '.', theme["game.visited"]); // FIXME: move out of the
                                                                  // loop
            }
        }
    }

    fn render_player(
        &self,
        maze_pos: Dims,
        game: &RunningGame,
        viewport: &mut Frame,
        theme: &Theme,
    ) {
        let player = self.sm_player_pos;
        let player_draw_pos = maze_pos + player.into();
        let cell = game
            .get_maze()
            .board
            .get_cell(self.data.game.get_player_pos())
            .unwrap();
        if !cell.get_wall(CellWall::Up) || !cell.get_wall(CellWall::Down) {
            viewport[player_draw_pos]
                .content_mut()
                .unwrap()
                .style
                .foreground_color = theme["game.player"].to_cross().foreground_color;
        } else {
            viewport.draw(player_draw_pos, self.data.player_char, theme["game.player"]);
        }
    }

    fn update_viewport(&mut self, data: &AppData) {
        if self.is_dpad_enabled() {
            let (viewport_rect, dpad_rect) = DPad::split_screen(data);
            let mut dpad_rect = dpad_rect;
            if data.settings.get_enable_margin_around_dpad() {
                dpad_rect = dpad_rect.margin(self.margins);
            }

            self.viewport_rect = viewport_rect;
            self.dpad_rect = Some(dpad_rect);
        } else {
            self.viewport_rect = Rect::sized(data.screen_size);
        }
    }
}
impl GameActivity {
    fn is_dpad_enabled(&self) -> bool {
        self.touch_controls.is_some()
    }

    fn init_dpad(&mut self, data: &AppData) {
        let dpad_type = DPadType::from_maze(self.data.game.get_maze());
        let swap_up_down = data.settings.get_dpad_swap_up_down();

        let touch_controls = DPad::new(None, swap_up_down, dpad_type);
        self.touch_controls = Some(Box::new(touch_controls));
    }

    fn update_dpad(&mut self, data: &AppData) {
        if (data.settings.get_enable_dpad() && data.settings.get_enable_mouse())
            != self.is_dpad_enabled()
        {
            if data.settings.get_enable_dpad() {
                log::info!("Enabling dpad");
                self.init_dpad(data);
            } else {
                log::info!("Disabling dpad");
                self.deinit_dpad(data);
            }
        }

        if self.is_dpad_enabled() {
            let dpad = self.touch_controls.as_mut().expect("dpad not set");

            dpad.swap_up_down = data.settings.get_dpad_swap_up_down();
            dpad.disable_highlight(!data.settings.get_enable_dpad_highlight());
        }
    }

    fn deinit_dpad(&mut self, data: &AppData) {
        self.touch_controls = None;

        self.viewport_rect = Rect::sized(data.screen_size);
        self.dpad_rect = None;
    }
}

impl ActivityHandler for GameActivity {
    fn update(&mut self, events: Vec<Event>, data: &mut AppData) -> Option<Change> {
        match self.data.game.get_state() {
            RunningGameState::NotStarted => self.data.game.start().unwrap(),
            RunningGameState::Paused => self.data.game.resume().unwrap(),
            _ => {}
        }

        self.update_dpad(data);
        self.update_viewport(data);

        if let Some(ref mut tc) = self.touch_controls {
            tc.update_space(self.dpad_rect.expect("dpad rect not set"));
        }

        for event in events {
            #[allow(clippy::single_match)]
            match event {
                Event::Term(event) => match event {
                    TermEvent::Key(key_event) => {
                        match self.data.handle_event(&data.settings, key_event) {
                            Err(false) => {
                                self.data.game.pause().unwrap();

                                return Some(Change::push(Activity::new_base_boxed(
                                    "pause".to_string(),
                                    PauseMenu::new(),
                                )));
                            }
                            Err(true) => return Some(Change::pop_until("main menu")),
                            Ok(_) => {}
                        }
                    }
                    TermEvent::Mouse(event) => {
                        if let Some(ref mut touch_controls) = self.touch_controls {
                            if let Some(dir) = touch_controls.apply_mouse_event(event) {
                                self.data.apply_move(&data.settings, dir, false);
                            }
                        }
                    }
                    _ => {}
                },
                _ => (),
            }
        }

        if let Some(ref mut tc) = self.touch_controls {
            tc.update_available_moves(if self.data.view_mode == GameViewMode::Adventure {
                self.data.game.get_available_moves()
            } else {
                [true; 6] // enable all
            });
        }

        if self.data.view_mode == GameViewMode::Adventure {
            match self.camera_mode {
                CameraMode::CloseFollow => {
                    self.data.camera_pos = maze2screen_3d(self.data.game.get_player_pos());
                }
                CameraMode::EdgeFollow { x: xoff, y: yoff } => 'b: {
                    self.data.camera_pos.2 = self.data.game.get_player_pos().2;

                    let (vp_size, does_fit) = self.viewport_size(data.screen_size);

                    if does_fit {
                        break 'b;
                    }

                    let xoff = xoff.to_abs(vp_size.0);
                    let yoff = yoff.to_abs(vp_size.1);

                    let player_pos = maze2screen(self.data.game.get_player_pos());
                    let player_pos_in_vp =
                        player_pos - self.data.camera_pos.into() + vp_size / 2 + Dims(1, 1);

                    if player_pos_in_vp.0 < xoff || player_pos_in_vp.0 > vp_size.0 - xoff {
                        self.data.camera_pos.0 = player_pos.0;
                    }

                    if player_pos_in_vp.1 < yoff || player_pos_in_vp.1 > vp_size.1 - yoff {
                        self.data.camera_pos.1 = player_pos.1;
                    }
                }
            }
        }

        self.sm_player_pos = lerp!((self.sm_player_pos) -> (maze2screen_3d(self.data.game.get_player_pos())) at data.settings.get_player_smoothing());
        self.sm_camera_pos = lerp!((self.sm_camera_pos) -> (self.data.camera_pos) at data.settings.get_camera_smoothing());

        self.show_debug = data.use_data.show_debug;

        if self.data.game.get_state() == RunningGameState::Finished {
            return Some(Change::replace_at(
                1,
                Activity::new_base_boxed(
                    "won".to_string(),
                    EndGamePopup::new(&self.data.game, self.data.maze_preset.clone()),
                ),
            ));
        };

        None
    }

    fn screen(&self) -> &dyn ui::Screen {
        self
    }
}

impl Screen for GameActivity {
    fn draw(&self, frame: &mut Frame, theme: &Theme) -> std::io::Result<()> {
        let maze_frame = self.current_floor_frame();
        let game = &self.data.game;

        let game_view_rect = self.viewport_rect;
        let game_view_size = game_view_rect.size();

        let (vp_size, does_fit) = self.viewport_size(game_view_size);
        let maze_pos = match does_fit {
            true => match self.data.view_mode {
                GameViewMode::Adventure => Dims(0, 0),
                GameViewMode::Spectator => maze2screen(Dims(0, 0)) - self.sm_camera_pos.into(),
            },
            false => vp_size / 2 - self.sm_camera_pos.into(),
        };

        // TODO: reuse the viewport between frames and resize it when needed
        let mut viewport = Frame::new(vp_size);

        // maze
        viewport.draw(maze_pos, maze_frame, ());
        self.render_visited_places(&mut viewport, maze_pos, theme);

        // player
        if (self.data.game.get_player_pos().2) == self.sm_camera_pos.2 {
            self.render_player(maze_pos, game, &mut viewport, theme);
        }

        // show viewport box
        let vp_pos = (game_view_size - vp_size) / 2 + self.viewport_rect.start;
        let vp_rect = Rect::sized_at(vp_pos, vp_size).margin(Dims(-1, -1));
        vp_rect.render(frame, theme["game.viewport.border"]);

        if let CameraMode::EdgeFollow { x: xoff, y: yoff } = self.camera_mode {
            if !does_fit && self.show_debug {
                render_edge_follow_rulers((xoff, yoff), frame, vp_rect, theme);
            }
        }

        self.render_meta_texts(frame, theme, vp_rect);

        frame.draw(vp_pos, &viewport, ());

        // touch controls
        if let Some(ref touch_controls) = self.touch_controls {
            let mut dpad_frame = Frame::new(self.dpad_rect.unwrap().size());

            touch_controls.render(&mut dpad_frame, theme);
            frame.draw(self.dpad_rect.unwrap().start, &dpad_frame, ());
        }

        if self.show_debug {
            if let Some(dpad_rect) = self.dpad_rect {
                dpad_rect.render(frame, theme["debug.border"]);
            }

            self.viewport_rect.render(frame, theme["debug.border"]);
        }

        Ok(())
    }
}

#[inline]
fn render_edge_follow_rulers(rulers: (Offset, Offset), frame: &mut Frame, vp: Rect, theme: &Theme) {
    let [s_start, s_end] = theme.extract(["debug.rulers.start", "debug.rulers.end"]);

    let vps = vp.size();

    let xo = rulers.0.to_abs(vps.0);
    let yo = rulers.1.to_abs(vps.1);

    let frame_pos = vp.start;

    use LineDir::{Horizontal, Vertical};
    const V: char = Vertical.round();
    const H: char = Horizontal.round();

    let mut draw = |pos, dir, end| {
        let style = match end {
            false => s_start,
            true => s_end,
        };
        frame.draw(frame_pos + pos, dir, style)
    };

    #[rustfmt::skip]
    {
        draw(Dims(xo        , 0        ), V, false);
        draw(Dims(vps.0 - xo, 0        ), V, true);
        draw(Dims(xo        , vps.1 + 1), V, false);
        draw(Dims(vps.0 - xo, vps.1 + 1), V, true);

        draw(Dims(0         , yo        ), H, false);
        draw(Dims(0         , vps.1 - yo), H, true);
        draw(Dims(vps.0 + 1 , yo        ), H, false);
        draw(Dims(vps.0 + 1 , vps.1 - yo), H, true);
    };
}

pub struct MazeBoard {
    frames: Vec<Frame>,
}

impl MazeBoard {
    pub fn new(game: &RunningGame, theme: &Theme) -> Self {
        let maze = game.get_maze();

        let mut frames: Vec<_> = (0..maze.size().2)
            .map(|floor| Self::render_floor(game, floor, theme))
            .collect();

        Self::render_special(&mut frames, game, theme);

        Self { frames }
    }

    fn render_floor(game: &RunningGame, floor: i32, theme: &Theme) -> Frame {
        let board = &game.get_maze().board;
        let normals = theme["game.walls"];

        let size = maze_render_size(board);

        let mut frame = Frame::new(size);
        frame.fill(renderer::Cell::styled(' ', theme["game.background"]));

        let mut draw = |pos, l: LineDir| frame.draw(Dims::from(pos), l.double(), normals);

        for y in -1..board.size().1 {
            for x in -1..board.size().0 {
                let cell_pos = Dims3D(x, y, floor);
                let Dims(rx, ry) = maze2screen(cell_pos);

                if board.get_wall(cell_pos, CellWall::Right) {
                    draw((rx + 1, ry), LineDir::Vertical);
                }

                if board.get_wall(cell_pos, CellWall::Bottom) {
                    draw((rx, ry + 1), LineDir::Horizontal);
                }

                let cp1 = cell_pos;
                let cp2 = cell_pos + Dims3D(1, 1, 0);

                let dir = LineDir::from_bools(
                    board.get_wall(cp1, CellWall::Bottom),
                    board.get_wall(cp1, CellWall::Right),
                    board.get_wall(cp2, CellWall::Top),
                    board.get_wall(cp2, CellWall::Left),
                );

                draw((rx + 1, ry + 1), dir);
            }
        }

        let layer = board.get_cells().layer(floor as usize).unwrap();
        Self::render_stairs(&mut frame, layer, game.get_maze().is_tower(), theme);

        frame
    }

    fn render_stairs(frame: &mut Frame, floors: Array2DView<Cell>, tower: bool, theme: &Theme) {
        let s_stairs_up = theme["game.stairs.up"];
        let s_stairs_down = theme["game.stairs.down"];
        let s_stairs_both = theme["game.stairs.both"];
        let s_stairs_up_tower = theme["game.stairs.up.tower"];

        for pos in floors.iter_pos() {
            let cell = floors[pos];
            let (up, down) = (!cell.get_wall(CellWall::Up), !cell.get_wall(CellWall::Down));
            let (ch, st) = match (up, down) {
                (true, true) => ('⥮', s_stairs_both),
                (true, false) => ('↑', s_stairs_up),
                (false, true) => ('↓', s_stairs_down),
                _ => continue,
            };

            let style = if tower && up { s_stairs_up_tower } else { st };
            let pos = maze2screen(pos);
            frame.draw(pos, ch, style);
        }
    }

    fn render_special(frames: &mut [Frame], game: &RunningGame, theme: &Theme) {
        let goal_style = theme["game.goal"];
        let goal_pos = game.get_goal_pos();

        let frame = &mut frames[goal_pos.2 as usize];
        frame.draw(maze2screen(goal_pos), '$', goal_style);
    }
}

pub fn game_theme_resolver() -> ThemeResolver {
    let mut resolver = ThemeResolver::new();

    resolver
        .link("game.walls", "border")
        // stairs
        .link("game.stairs", "game.walls")
        .link("game.stairs.up", "game.stairs")
        .link("game.stairs.down", "game.stairs")
        .link("game.stairs.both", "game.stairs")
        .link("game.stairs.up.tower", "game.goal")
        // game
        .link("game.goal", "")
        .link("game.player", "highlight")
        .link("game.player.on.stairs", "game.stairs")
        .link("game.visited", "dim")
        .link("game.background", "background")
        // special
        .link("game.viewport.border", "border")
        .link("debug.border", "border")
        .link("debug.rulers", "debug.border")
        .link("debug.rulers.start", "debug.rulers")
        .link("debug.rulers.end", "debug.rulers");

    resolver
}
