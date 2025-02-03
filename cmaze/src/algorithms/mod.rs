pub mod region_generator;
pub mod region_splitter;
pub mod types;

use hashbrown::{HashMap, HashSet};
use rand::{seq::SliceRandom as _, thread_rng, Rng as _, SeedableRng as _};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

use std::{iter, sync::Arc};

use crate::{
    array::Array3D,
    dims::*,
    gameboard::{maze::MazeBoard, Cell, CellWall, Maze},
    progress::ProgressHandle,
    registry::Registry,
};
use region_generator::RegionGenerator;
use region_splitter::RegionSplitter;
pub use types::*;

/// Random number generator used for anything, where determinism is required.
pub type Random = rand_xoshiro::Xoshiro256StarStar;

/// Registry of the region generators.
pub type GeneratorRegistry = Registry<dyn RegionGenerator>;

/// Registry of the region splitters.
pub type SplitterRegistry = Registry<dyn RegionSplitter>;

#[derive(Debug)]
pub enum GeneratorError {
    Unknown,
    Validation,
}

#[derive(Debug, Clone)]
enum LocalSplitterSpec {
    Predefined {
        regions: Array3D<Option<u8>>,
        region_specs: Vec<LocalRegionSpec>,
    },
    ToGenerate {
        mask: CellMask,
        splitter: (Arc<dyn RegionSplitter>, Params),
        generator: (Arc<dyn RegionGenerator>, Params),
    },
}

#[derive(Debug, Clone)]
enum LocalRegionSpec {
    Predefined(MazeBoard),
    ToGenerate {
        generator: Arc<dyn RegionGenerator>,
        params: Params,
        seed: Option<u64>,
    },
}

#[derive(Debug, Clone)]
enum PosInMaze {
    Region(u8),
    Cell(Dims3D),
}

impl From<Position> for PosInMaze {
    fn from(pos: Position) -> Self {
        match pos {
            Position::Region(id) => PosInMaze::Region(id),
            Position::Pos(pos) => PosInMaze::Cell(pos),
        }
    }
}

impl From<Dims3D> for PosInMaze {
    fn from(pos: Dims3D) -> Self {
        PosInMaze::Cell(pos)
    }
}

/// Main struct of this module.
///
/// It generates the complete maze from specification and from the optional seed. For more info
/// check out [`LocalSplitterSpec`], [`LocalRegionSpec`] and [`MazeType`].
#[derive(Debug, Clone)]
pub struct Generator {
    seed: Option<u64>,
    splitter: LocalSplitterSpec,
    type_: MazeType,
    start: Option<PosInMaze>,
    end: Option<PosInMaze>,
}

impl Generator {
    pub fn from_maze_spec(
        spec: &MazeSpec,
        generators: &GeneratorRegistry,
        splitters: &SplitterRegistry,
    ) -> Self {
        fn get_from_registry<T: ?Sized>(
            registry: &Registry<T>,
            pair: Option<&(String, Params)>,
        ) -> (Arc<T>, Params) {
            pair.as_ref()
                .map(|(name, params)| (registry.get(name).unwrap(), params.clone()))
                .unwrap_or_else(|| (registry.get_default().unwrap(), Params::default()))
        }

        let MazeSpec {
            inner_spec,
            seed,
            maze_type,
        } = spec;

        match inner_spec {
            MazeSpecType::Regions {
                regions,
                start,
                end,
            } => {
                let size = regions.first().unwrap().mask.size();
                let mut region_ids = Array3D::new_dims(None, size).unwrap();

                let region_specs = regions
                    .iter()
                    .enumerate()
                    .map(|(region_id, MazeRegionSpec { mask, region_type })| {
                        for pos in mask.iter_enabled() {
                            region_ids[pos] = Some(region_id as u8);
                        }
                        match region_type {
                            MazeRegionType::Predefined { board } => {
                                LocalRegionSpec::Predefined(board.clone())
                            }
                            MazeRegionType::Generated {
                                generator: (generator, params),
                                seed,
                            } => LocalRegionSpec::ToGenerate {
                                generator: generators.get(generator).expect("unknown generator"),
                                params: params.clone(),
                                seed: *seed,
                            },
                        }
                    })
                    .collect::<Vec<_>>();

                Self {
                    seed: *seed,
                    splitter: LocalSplitterSpec::Predefined {
                        regions: region_ids,
                        region_specs,
                    },
                    type_: maze_type.unwrap_or(MazeType::Normal),
                    start: start.map(Into::into),
                    end: end.map(Into::into),
                }
            }
            MazeSpecType::Simple {
                start,
                end,
                size,
                mask,
                splitter,
                generator,
            } => {
                let size = size.or_else(|| mask.as_ref().map(|m| m.size())).unwrap();
                Self {
                    seed: *seed,
                    splitter: LocalSplitterSpec::ToGenerate {
                        mask: mask
                            .clone()
                            .unwrap_or_else(|| CellMask::new_dims(size).unwrap()),
                        splitter: get_from_registry(splitters, splitter.as_ref()),
                        generator: get_from_registry(generators, generator.as_ref()),
                    },
                    type_: maze_type.unwrap_or(MazeType::default()),
                    start: start.map(Into::into),
                    end: end.map(Into::into),
                }
            }
        }
    }

    pub fn generate(&self, progress: ProgressHandle) -> Result<Maze, GeneratorError> {
        let seed = self.seed.unwrap_or_else(|| thread_rng().gen());
        let mut rng = Random::seed_from_u64(seed);

        Ok(match &self.splitter {
            LocalSplitterSpec::Predefined {
                regions,
                region_specs,
            } => {
                let mask = regions.clone().to_mask();
                progress.lock().from = mask.enabled_count();

                let regions = regions.clone();
                let region_count = region_specs.len();
                let generated_regions: Vec<_> = region_specs
                    .clone()
                    .into_par_iter()
                    .zip(split_rng(&mut rng, region_count))
                    .enumerate()
                    .map(|(i, (spec, mut rng))| match spec {
                        LocalRegionSpec::Predefined(maze) => Some(maze),
                        LocalRegionSpec::ToGenerate {
                            generator,
                            params,
                            seed,
                        } => {
                            let region_mask =
                                CellMask::from(regions.clone().map(|r| r == Some(i as u8)));
                            if let Some(seed) = seed {
                                rng = Random::seed_from_u64(seed);
                            }
                            generator.generate(region_mask, &mut rng, progress.split(), &params)
                        }
                    })
                    .collect::<Option<_>>()
                    .ok_or(GeneratorError::Unknown)?;

                let regions = regions.map(|r| r.unwrap_or_default());
                let board = Self::connect_regions(&regions, &mask, generated_regions, &mut rng);

                let start = match self.start {
                    None => mask.iter_enabled().next().unwrap(),
                    Some(PosInMaze::Region(id)) => Self::mask_of_region(id, &regions, &mask)
                        .random_cell(&mut rng)
                        .unwrap(),
                    Some(PosInMaze::Cell(pos)) => pos,
                };

                let end = match self.end {
                    None => mask.iter_enabled().last().unwrap(),
                    Some(PosInMaze::Region(id)) => Self::mask_of_region(id, &regions, &mask)
                        .random_cell(&mut rng)
                        .unwrap(),
                    Some(PosInMaze::Cell(pos)) => pos,
                };

                Maze {
                    board,
                    type_: self.type_,
                    start,
                    end,
                }
            }
            LocalSplitterSpec::ToGenerate {
                mask,
                splitter: (splitter, split_args),
                generator: (generator, gen_args),
            } => {
                progress.lock().from = generator.guess_progress_complexity(mask);

                const SPLIT_COUNT: usize = 100;

                let parts = match self.type_ {
                    MazeType::Normal => vec![mask.clone()],
                    MazeType::Tower => mask
                        .as_array3d()
                        .layers()
                        .map(|l| l.to_array().into())
                        .collect(),
                };

                let mut groups = Array3D::new_dims(0, mask.size()).unwrap();
                for (i, part) in parts.iter().enumerate() {
                    for pos in part.iter_enabled() {
                        groups[pos] = i.try_into().expect("too many parts (floors)");
                    }
                }

                let parts = parts
                    .into_par_iter()
                    .zip(split_rng(&mut rng, SPLIT_COUNT))
                    .map(|(mask, mut rng)| {
                        let group_count =
                            (mask.enabled_count() / SPLIT_COUNT).clamp(1, u8::MAX as usize) as u8;
                        let groups = splitter
                            .split(&mask, &mut rng, progress.split(), split_args)
                            .ok_or(GeneratorError::Unknown)?;
                        let masks = Self::split_to_masks(group_count, &groups, &mask);

                        if progress.is_stopped() {
                            return Err(GeneratorError::Unknown);
                        }

                        let progresses = masks
                            .iter()
                            .map(|mask| {
                                let local = progress.split();
                                local.lock().from = generator.guess_progress_complexity(mask);
                                local
                            })
                            .collect::<Vec<_>>();
                        progress.lock().from = 0;

                        let Some(regions) = masks
                            .into_par_iter()
                            .zip(progresses)
                            .zip(split_rng(&mut rng, group_count as usize))
                            .map(|((mask, progress), mut rng)| {
                                generator.generate(mask, &mut rng, progress, gen_args)
                            })
                            .collect()
                        else {
                            return Err(GeneratorError::Unknown);
                        };

                        Ok(Self::connect_regions(&groups, &mask, regions, &mut rng))
                    })
                    .collect::<Result<_, GeneratorError>>()?;

                progress.lock().finish();

                let board = match self.type_ {
                    MazeType::Normal => Self::connect_regions(&groups, mask, parts, &mut rng),
                    MazeType::Tower => {
                        let mut board = MazeBoard {
                            cells: Array3D::new_dims(Cell::new(), mask.size()).unwrap(),
                            mask: mask.clone(),
                        };
                        for pos in mask.iter_enabled() {
                            board.cells[pos] = parts[pos.2 as usize].cells[Dims3D(pos.0, pos.1, 0)];
                        }

                        board
                    }
                };

                let start = match self.start {
                    None => mask.iter_enabled().next().unwrap(),
                    Some(PosInMaze::Region(_)) => return Err(GeneratorError::Validation),
                    Some(PosInMaze::Cell(pos)) => pos,
                };

                let end = match self.end {
                    None => mask.iter_enabled().last().unwrap(),
                    Some(PosInMaze::Region(_)) => return Err(GeneratorError::Validation),
                    Some(PosInMaze::Cell(pos)) => pos,
                };

                Maze {
                    board,
                    type_: self.type_,
                    start,
                    end,
                }
            }
        })
    }

    // Split groups into masks, ready for maze generation
    pub fn split_to_masks(group_count: u8, groups: &Array3D<u8>, mask: &CellMask) -> Vec<CellMask> {
        let mut masks =
            vec![CellMask::new_dims_empty(groups.size()).unwrap(); group_count as usize];

        for (cell, &group) in groups.iter_pos().zip(groups.iter()) {
            if mask[cell] {
                masks[group as usize][cell] = true;
            }
        }

        masks
    }

    // Get mask of the single region
    pub fn mask_of_region(region: u8, groups: &Array3D<u8>, mask: &CellMask) -> CellMask {
        let mut reg_mask = CellMask::new_dims_empty(groups.size()).unwrap();

        for (cell, &group) in groups.iter_pos().zip(groups.iter()) {
            if group == region && mask[cell] {
                reg_mask[cell] = true;
            }
        }

        reg_mask
    }

    pub fn connect_regions(
        groups: &Array3D<u8>,
        mask: &CellMask,
        regions: Vec<MazeBoard>,
        rng: &mut Random,
    ) -> MazeBoard {
        // Disclaimer: this implementation can be slow af, since there is a maximum of a 256 groups
        // We use a simple Kruskal's algorithm to connect the regions

        let mut walls = HashMap::new();
        for (pair, way) in Self::build_region_graph(groups, mask) {
            assert!(pair.0 < pair.1);
            walls.entry(pair).or_insert_with(Vec::new).push(way);
        }

        // Choose only one wall from all of the pairs
        let mut walls: Vec<_> = walls
            .into_iter()
            .map(|(k, v)| (k, *v.choose(rng).unwrap()))
            .collect();
        walls.shuffle(rng);

        let mut sets: Vec<HashSet<u8>> = (0..regions.len() as u8)
            .map(|i| iter::once(i).collect())
            .collect();

        // Combine the regions, so we can start connecting them
        let mut board = MazeBoard {
            cells: Array3D::new_dims(Cell::new(), groups.size()).unwrap(),
            mask: mask.clone(),
        };
        for cell in groups.iter_pos() {
            let group = groups[cell];
            let region = &regions[group as usize];
            board.cells[cell] = region.cells[cell];
        }

        #[allow(unused_variables)]
        while let Some(((from_g, to_g), (from, dir))) = walls.pop() {
            let from_set = sets
                .iter()
                .enumerate()
                .find(|(_, set)| set.contains(&from_g))
                .unwrap();
            if from_set.1.contains(&to_g) {
                continue;
            }
            board.remove_wall(from, dir);

            let from_set = sets.swap_remove(from_set.0);
            let to_set = sets.iter_mut().find(|set| set.contains(&to_g)).unwrap();
            to_set.extend(from_set);
        }

        board
    }

    pub fn build_region_graph(
        groups: &Array3D<u8>,
        mask: &CellMask,
    ) -> Vec<((u8, u8), (Dims3D, CellWall))> {
        let mut borders = vec![];

        for cell in mask.iter_enabled() {
            let group = groups[cell];

            use CellWall::*;
            for dir in [Right, Bottom, Down] {
                let neighbor = cell + dir.to_coord();
                if !mask[neighbor] {
                    continue;
                }

                if let Some(&neighbor_group) = groups.get(neighbor) {
                    if neighbor_group != group {
                        borders.push((
                            (group.min(neighbor_group), group.max(neighbor_group)),
                            (cell, dir),
                        ));
                    }
                }
            }
        }

        borders
    }
}

fn split_rng(
    rng: &mut Random,
    count: usize,
) -> impl IndexedParallelIterator<Item = Random> + use<'_> {
    (0..count).into_par_iter().map(|_| {
        let mut rng = rng.clone();
        rng.long_jump();
        rng
    })
}
