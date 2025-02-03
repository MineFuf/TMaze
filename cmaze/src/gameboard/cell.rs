use serde::{Deserialize, Serialize};

use crate::{dims::*, gameboard::cell::CellWall::*};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Cell {
    left: bool,
    top: bool,
    right: bool,
    bottom: bool,
    up: bool,
    down: bool,
}

impl Cell {
    pub fn new() -> Cell {
        Cell {
            left: true,
            right: true,
            top: true,
            bottom: true,
            up: true,
            down: true,
        }
    }

    pub fn remove_wall(&mut self, wall: CellWall) {
        match wall {
            Left => self.left = false,
            Top => self.top = false,
            Right => self.right = false,
            Bottom => self.bottom = false,
            Up => self.up = false,
            Down => self.down = false,
        }
    }

    pub fn get_wall(&self, wall: CellWall) -> bool {
        match wall {
            Left => self.left,
            Top => self.top,
            Right => self.right,
            Bottom => self.bottom,
            Up => self.up,
            Down => self.down,
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone)]
pub enum CellWall {
    Left,
    Right,
    Top,
    Bottom,
    Up,
    Down,
}

impl CellWall {
    pub fn to_coord(&self) -> Dims3D {
        match self {
            Self::Left => Dims3D(-1, 0, 0),
            Self::Right => Dims3D(1, 0, 0),
            Self::Top => Dims3D(0, -1, 0),
            Self::Bottom => Dims3D(0, 1, 0),
            Self::Up => Dims3D(0, 0, 1),
            Self::Down => Dims3D(0, 0, -1),
        }
    }

    pub fn reverse_wall(&self) -> CellWall {
        match self {
            Left => Right,
            Right => Left,
            Top => Bottom,
            Bottom => Top,
            Up => Down,
            Down => Up,
        }
    }

    pub fn perpendicular_walls(&self) -> (CellWall, CellWall, CellWall, CellWall) {
        match self {
            Left | Right => (Top, Bottom, Up, Down),
            Top | Bottom => (Left, Right, Up, Down),
            Up | Down => (Top, Bottom, Left, Right),
        }
    }

    pub fn get_in_order() -> [CellWall; 6] {
        [Top, Left, Right, Bottom, Up, Down]
    }
}
