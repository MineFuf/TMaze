use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use self::CellWall::*;
use crate::{
    algorithms::{CellMask, MazeType},
    array::Array3D,
    dims::*,
    gameboard::cell::{Cell, CellWall},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MazeBoard {
    pub(crate) cells: Array3D<Cell>,
    pub(crate) mask: CellMask,
}

impl MazeBoard {
    pub fn size(&self) -> Dims3D {
        self.cells.size()
    }

    pub fn is_in_bounds(&self, pos: Dims3D) -> bool {
        0 <= pos.0
            && pos.0 < self.size().0
            && 0 <= pos.1
            && pos.1 < self.size().1
            && 0 <= pos.2
            && pos.2 < self.size().2
    }

    pub fn is_valid_neighbor(&self, cell: Dims3D, off: Dims3D) -> bool {
        (off.0 == -1 || off.0 == 1 || off.0 == 0)
            && (off.1 == -1 || off.1 == 1 || off.1 == 0)
            && (off.2 == -1 || off.2 == 1 || off.2 == 0)
            && ((off.0 == 1 || off.0 == -1) as u8
                + (off.1 == 1 || off.1 == -1) as u8
                + (off.2 == 1 || off.2 == -1) as u8)
                == 1
            && self.is_in_bounds(cell)
            && self.is_in_bounds(Dims3D(cell.0 + off.0, cell.1 + off.1, cell.2 + off.2))
    }

    pub fn is_valid_wall(&self, cell: Dims3D, wall: CellWall) -> bool {
        let neighbor_offset = wall.to_coord();
        self.is_valid_neighbor(cell, neighbor_offset)
    }

    pub fn which_wall_between(cell: Dims3D, cell2: Dims3D) -> Option<CellWall> {
        match (cell.0 - cell2.0, cell.1 - cell2.1, cell.2 - cell2.2) {
            (-1, 0, 0) => Some(Right),
            (1, 0, 0) => Some(Left),
            (0, -1, 0) => Some(Bottom),
            (0, 1, 0) => Some(Top),
            (0, 0, 1) => Some(Down),
            (0, 0, -1) => Some(Up),
            _ => None,
        }
    }

    pub fn get_wall(&self, from: Dims3D, wall: CellWall) -> bool {
        let to = from + wall.to_coord();

        if !self.mask[from] || !self.mask[to] {
            return self.mask[from] || self.mask[to];
        }

        self.get_cell(from)
            .map(|c| c.get_wall(wall))
            .unwrap_or(false)
    }

    pub fn get_neighbors_pos(&self, cell: Dims3D) -> SmallVec<[Dims3D; 6]> {
        CellWall::get_in_order()
            .into_iter()
            .map(|wall| CellWall::to_coord(&wall))
            .filter(|off| self.is_valid_neighbor(cell, *off))
            .map(|off| cell + off)
            .collect()
    }

    pub fn get_neighbors(&self, cell: Dims3D) -> SmallVec<[&Cell; 6]> {
        self.get_neighbors_pos(cell)
            .into_iter()
            .filter_map(|pos| self.get_cell(pos))
            .collect()
    }

    pub fn remove_wall(&mut self, cell: Dims3D, wall: CellWall) {
        if !self.is_valid_wall(cell, wall) {
            return;
        }

        self.cells[cell].remove_wall(wall);
        self.cells[cell + wall.to_coord()].remove_wall(wall.reverse_wall());
    }

    pub fn get_cells(&self) -> &Array3D<Cell> {
        &self.cells
    }

    pub fn get_cell(&self, pos: Dims3D) -> Option<&Cell> {
        self.cells.get(pos)
    }

    pub fn get_cell_mut(&mut self, pos: Dims3D) -> Option<&mut Cell> {
        if self.is_in_bounds(pos) {
            Some(&mut self.cells[pos])
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Maze {
    pub board: MazeBoard,
    pub(crate) type_: MazeType,
    pub start: Dims3D,
    pub end: Dims3D,
}

impl Maze {
    pub fn size(&self) -> Dims3D {
        self.board.size()
    }

    pub fn is_tower(&self) -> bool {
        self.type_ == MazeType::Tower
    }
}
