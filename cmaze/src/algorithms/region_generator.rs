use std::fmt;

use hashbrown::HashSet;
use rand::seq::SliceRandom as _;
use smallvec::SmallVec;

use crate::{
    array::Array3D,
    gameboard::{maze::MazeBoard, Cell, CellWall},
    progress::ProgressHandle,
};

use super::{CellMask, Dims3D,  Params, Random};

pub trait RegionGenerator: fmt::Debug + Sync + Send {
    fn generate(
        &self,
        mask: CellMask,
        rng: &mut Random,
        progress: ProgressHandle,
        params: &Params,
    ) -> Option<MazeBoard>;

    fn guess_progress_complexity(&self, mask: &CellMask) -> usize {
        mask.enabled_count()
    }
}

#[derive(Debug)]
pub struct DepthFirstSearch;

impl RegionGenerator for DepthFirstSearch {
    fn generate(
        &self,
        mask: CellMask,
        rng: &mut Random,
        progress: ProgressHandle,
        params: &Params,
    ) -> Option<MazeBoard> {
        let size = mask.size();

        let cells = Array3D::new_dims(Cell::new(), size).unwrap();
        let mut board = MazeBoard { cells, mask };

        // Maybe write a funky macro on struct, that loads all fields from params ?
        let no_rng = params.parsed_or_warn("no_rng", false);

        progress.lock().from = board.mask.enabled_count();

        let mut visited = HashSet::with_capacity(board.mask.enabled_count());
        let mut stack = Vec::new();

        let mut current = if no_rng {
            board.mask.iter_enabled().next().unwrap()
        } else {
            board.mask.random_cell(rng).unwrap()
        };

        visited.insert(current);
        stack.push(current);
        while !stack.is_empty() {
            current = stack.pop().unwrap();
            let unvisited_neighbors = board
                .get_neighbors_pos(current)
                .into_iter()
                .filter(|cell| board.mask[*cell])
                .filter(|cell| !visited.contains(cell))
                .collect::<SmallVec<[_; 6]>>();

            if !unvisited_neighbors.is_empty() {
                stack.push(current);
                let next = if no_rng {
                    unvisited_neighbors[0]
                } else {
                    *unvisited_neighbors.choose(rng).unwrap()
                };
                let chosen_wall = MazeBoard::which_wall_between(current, next).unwrap();
                board.remove_wall(current, chosen_wall);
                visited.insert(next);
                stack.push(next);
            }

            progress.lock().done = visited.len();
            if progress.is_stopped() {
                return None;
            }
        }

        progress.lock().finish();

        Some(board)
    }
}

#[derive(Debug)]
pub struct RndKruskals;

impl RegionGenerator for RndKruskals {
    fn generate(
        &self,
        mask: CellMask,
        rng: &mut Random,
        progress: ProgressHandle,
        params: &Params,
    ) -> Option<MazeBoard> {
        let Dims3D(w, h, d) = mask.size();
        let (wu, hu, du) = (w as usize, h as usize, d as usize);

        let mut walls: Vec<(Dims3D, CellWall)> = Vec::new();
        let mut sets = Vec::<HashSet<Dims3D>>::new();
        for pos in Dims3D::iter_fill(Dims3D::ZERO, mask.size()) {
            use CellWall::*;

            if mask[pos] {
                if mask[pos + Dims3D(1, 0, 0)] {
                    walls.push((pos, Right));
                }

                if mask[pos + Dims3D(0, 1, 0)] {
                    walls.push((pos, Bottom));
                }

                if mask[pos + Dims3D(0, 0, 1)] {
                    walls.push((pos, Up));
                }

                sets.push(Some(pos).into_iter().collect());
            }
        }

        let starter_wall_count = walls.len();
        progress.lock().from = walls.len();

        let cells: Array3D<Cell> = Array3D::new(Cell::new(), wu, hu, du);

        let mut maze = MazeBoard { cells, mask };

        if !params.parsed_or_warn("no_rng", false) {
            walls.shuffle(rng);
        }

        while let Some((from, wall)) = walls.pop() {
            let to = from + wall.to_coord();

            let from_set = sets.iter().position(|set| set.contains(&from)).unwrap();

            if sets[from_set].contains(&to) {
                continue;
            }

            maze.remove_wall(from, wall);
            let from_set = sets.swap_remove(from_set);

            let to_set = sets.iter().position(|set| set.contains(&to)).unwrap();
            sets[to_set].extend(from_set);

            progress.lock().done = starter_wall_count - walls.len();
            if progress.is_stopped() {
                return None;
            }
        }

        progress.lock().finish();

        Some(maze)
    }
}
