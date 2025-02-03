use crate::{
    algorithms::{
        Generator, GeneratorError, GeneratorRegistry, MazeSpec, MazeType, SplitterRegistry,
    },
    dims::*,
    gameboard::{CellWall, Maze},
    progress::{Progress, ProgressHandle},
};

use pausable_clock::{PausableClock, PausableInstant};

use std::{
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Debug)]
pub struct GameAlreadyRunningError {}
#[derive(Debug)]
pub struct GameNotRunningError {}
#[derive(Debug)]
pub struct GameNotPausedError {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RunningGameState {
    NotStarted,
    Running,
    Paused,
    Finished,
    Quitted,
}

#[derive(Debug)]
pub struct GameProperities {
    pub maze_spec: MazeSpec,
}

pub enum MoveMode {
    Slow,
    Normal,
    Fast,
}

pub struct RunningJob<R> {
    pub handle: JoinHandle<R>,
    pub progress: ProgressHandle,
}

impl<R> RunningJob<R> {
    pub fn progress(&self) -> Progress {
        self.progress.progress()
    }
}

pub struct RunningGame {
    maze: Maze,
    state: RunningGameState,
    maze_spec: MazeSpec,
    #[allow(dead_code)]
    clock: Option<PausableClock>,
    start: Option<PausableInstant>,
    player_pos: Dims3D,
    moves: Vec<(Dims3D, CellWall)>,
}

impl RunningGame {
    #[allow(unused_variables)]
    pub fn prepare(
        props: GameProperities,
        gen_registry: &GeneratorRegistry,
        splitter_registry: &SplitterRegistry,
    ) -> Result<RunningJob<Option<RunningGame>>, GeneratorError> {
        if !props.maze_spec.validate(gen_registry, splitter_registry) {
            return Err(GeneratorError::Validation);
        }

        let GameProperities { maze_spec } = props;

        let generator = Generator::from_maze_spec(&maze_spec, gen_registry, splitter_registry);

        let progress = ProgressHandle::new();
        let progress_clone = progress.clone();

        let handle = thread::spawn(move || {
            let maze = generator.generate(progress_clone).ok()?;

            Some(RunningGame {
                player_pos: maze.start,
                maze,
                state: RunningGameState::NotStarted,
                maze_spec,
                clock: None,
                start: None,
                moves: vec![],
            })
        });

        Ok(RunningJob { handle, progress })
    }

    pub fn get_state(&self) -> RunningGameState {
        self.state
    }

    pub fn get_maze(&self) -> &Maze {
        &self.maze
    }

    pub fn get_player_pos(&self) -> Dims3D {
        self.player_pos
    }

    pub fn get_goal_pos(&self) -> Dims3D {
        self.maze.end
    }

    pub fn get_moves(&self) -> &Vec<(Dims3D, CellWall)> {
        &self.moves
    }

    pub fn get_move_count(&self) -> usize {
        self.moves.len()
    }

    pub fn get_basic_game_spec(&self) -> &MazeSpec {
        &self.maze_spec
    }

    pub fn get_available_moves(&self) -> [bool; 6] {
        let cell = &self.maze.board.get_cell(self.player_pos).unwrap();
        CellWall::get_in_order().map(|wall| !cell.get_wall(wall))
    }

    pub fn start(&mut self) -> Result<(), GameAlreadyRunningError> {
        if let RunningGameState::NotStarted = self.get_state() {
            self.state = RunningGameState::Running;
            self.clock = Some(PausableClock::default());
            self.start = Some(self.clock.as_mut().unwrap().now());

            Ok(())
        } else {
            Err(GameAlreadyRunningError {})
        }
    }

    pub fn quit(&mut self) {
        self.state = RunningGameState::Quitted;
        self.clock = None;
        self.start = None;
    }

    pub fn move_player(
        &mut self,
        dir: CellWall,
        move_mode: MoveMode,
        tower_auto_up: bool,
    ) -> Result<(Dims3D, usize), GameNotRunningError> {
        self.check_running()?;

        let mut count = 0;

        match move_mode {
            MoveMode::Slow => {
                return if self
                    .maze
                    .board
                    .get_cell(self.player_pos)
                    .unwrap()
                    .get_wall(dir)
                {
                    Ok((self.player_pos, 0))
                } else {
                    self.moves.push((self.player_pos, dir));
                    self.player_pos += dir.to_coord();
                    Ok((self.player_pos, 1))
                }
            }

            MoveMode::Fast => {
                while !self
                    .maze
                    .board
                    .get_cell(self.player_pos)
                    .unwrap()
                    .get_wall(dir)
                {
                    self.moves.push((self.player_pos, dir));
                    self.player_pos += dir.to_coord();
                    count += 1;
                }
            }

            MoveMode::Normal => loop {
                let mut cell = self.maze.board.get_cell(self.player_pos).unwrap();

                if cell.get_wall(dir) {
                    break;
                }

                count += 1;

                self.moves.push((self.player_pos, dir));
                self.player_pos += dir.to_coord();

                cell = self.maze.board.get_cell(self.player_pos).unwrap();

                let perps = dir.perpendicular_walls();
                if !cell.get_wall(perps.0)
                    || !cell.get_wall(perps.1)
                    || !cell.get_wall(perps.2)
                    || !cell.get_wall(perps.3)
                {
                    break;
                }
            },
        }

        if tower_auto_up
            && self.maze.type_ == MazeType::Tower
            && !self
                .maze
                .board
                .get_cell(self.player_pos)
                .unwrap()
                .get_wall(CellWall::Up)
        {
            self.moves.push((self.player_pos, CellWall::Up));
            self.player_pos += CellWall::Up.to_coord();
            count += 1;
        }

        if self.player_pos == self.maze.end {
            self.state = RunningGameState::Finished;
            self.clock.as_mut().unwrap().pause();
        }

        Ok((self.player_pos, count))
    }

    pub fn check_running(&self) -> Result<(), GameNotRunningError> {
        match self.state {
            RunningGameState::Running => Ok(()),
            _ => Err(GameNotRunningError {}),
        }
    }

    pub fn check_paused(&self) -> Result<(), GameNotPausedError> {
        match self.state {
            RunningGameState::Paused => Ok(()),
            _ => Err(GameNotPausedError {}),
        }
    }

    pub fn get_elapsed(&self) -> Option<Duration> {
        self.clock.as_ref().map(|c| self.start.unwrap().elapsed(c))
    }

    pub fn pause(&mut self) -> Result<(), GameNotRunningError> {
        self.check_running()?;

        self.state = RunningGameState::Paused;
        self.clock.as_mut().unwrap().pause();

        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), GameNotPausedError> {
        self.check_paused()?;

        self.state = RunningGameState::Running;
        self.clock.as_mut().unwrap().resume();

        Ok(())
    }

    pub fn reset(&mut self) {
        self.state = RunningGameState::NotStarted;
        self.moves.clear();
        self.player_pos = Dims3D(0, 0, 0);

        self.clock = None;
        self.start = None;
    }
}
