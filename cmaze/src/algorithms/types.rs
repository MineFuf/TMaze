use std::{ops, str::FromStr};

use hashbrown::HashMap;
use rand::{seq::SliceRandom as _, Rng as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{array::Array3D, gameboard::{maze::MazeBoard, CellWall}};

use super::{Dims3D, GeneratorRegistry, Random, SplitterRegistry};

/// Parameters for different algorithms. Region splitter, region generator, etc.
/// In the future, not only String will be allowed, but also other types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Params {
    map: HashMap<String, String>,
}

impl Params {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(|s| s.as_str())
    }

    pub fn parsed<T: FromStr>(&self, key: &str) -> Option<Result<T, T::Err>> {
        self.get(key).map(|s| s.parse())
    }

    pub fn parsed_or<T: FromStr>(&self, key: &str, default: T) -> T {
        match self.parsed(key) {
            None | Some(Err(_)) => default,
            Some(Ok(v)) => v,
        }
    }

    pub fn parsed_or_warn<T: FromStr>(&self, key: &str, default: T) -> T {
        match self.parsed(key) {
            None => default,
            Some(Ok(v)) => v,
            Some(Err(_)) => {
                log::warn!("Invalid value for parameter '{}', using default value", key);
                default
            }
        }
    }
}

/// Specific algorithm specification.
pub type Algorithm = (String, Params);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MazeSpec {
    // /// Size of the maze.
    // pub size: Dims3D,
    /// Specification of the maze.
    #[serde(default, flatten)]
    pub inner_spec: MazeSpecType,

    /// Seed of the maze.
    ///
    /// Used for deterministic generation.
    pub seed: Option<u64>,

    /// Type of the maze.
    pub maze_type: Option<MazeType>,
}

impl MazeSpec {
    pub fn validate(&self, generators: &GeneratorRegistry, splitters: &SplitterRegistry) -> bool {
        fn check_position(pos: Position, regions: &[MazeRegionSpec]) -> bool {
            match pos {
                Position::Pos(pos) => {
                    // Is the position at least in one region?
                    regions.iter().any(|r| r.mask[pos])
                }
                Position::Region(region) => {
                    // Is the region valid?
                    (region as usize) < regions.len()
                }
            }
        }

        fn pos_to_dim(pos: Position, regions: &[MazeRegionSpec]) -> Dims3D {
            match pos {
                Position::Pos(pos) => pos,
                Position::Region(region) => {
                    regions[region as usize].mask.iter_enabled().next().unwrap()
                }
            }
        }

        match &self.inner_spec {
            MazeSpecType::Regions {
                regions,
                start,
                end,
            } => {
                if let (Some(Position::Pos(start)), Some(Position::Pos(end))) = (start, end) {
                    if start == end {
                        return false;
                    }
                }

                if regions.is_empty() {
                    return false;
                }

                let size = regions.first().unwrap().mask.size();
                if regions.iter().any(|r| !r.validate(size, generators)) {
                    return false;
                }

                let mut exclusion_check_mask = CellMask::new_dims_empty(size).unwrap();
                for region in regions {
                    for pos in region.mask.iter_enabled() {
                        if exclusion_check_mask[pos] {
                            return false;
                        }

                        exclusion_check_mask[pos] = true;
                    }
                }

                if let Some(start) = *start {
                    if !check_position(start, regions) {
                        return false;
                    }
                }

                if let Some(end) = *end {
                    if !check_position(end, regions) {
                        return false;
                    }
                }

                if let (Some(start), Some(end)) = (start, end) {
                    let union = regions
                        .iter()
                        .fold(CellMask::new_dims_empty(size).unwrap(), |a, b| {
                            a.or(&b.mask)
                        });
                    if !union.connactable(pos_to_dim(*start, regions), pos_to_dim(*end, regions)) {
                        return false;
                    }
                }
            }

            MazeSpecType::Simple {
                size,
                start,
                end,
                mask,
                generator,
                splitter,
            } => {
                if let (Some(start), Some(end)) = (start, end) {
                    if start == end {
                        return false;
                    }
                }

                let Some(size) = size.or_else(|| mask.as_ref().map(|m| m.size())) else {
                    return false;
                };

                if !size.all_positive() {
                    return false;
                }

                if let Some(mask) = mask {
                    if mask.size() != size {
                        return false;
                    }

                    if mask.is_empty() {
                        return false;
                    }

                    if mask.enabled_count() < 2 {
                        return false;
                    }

                    if !mask.is_connected() {
                        return false;
                    }

                    if let Some(start) = start {
                        if !mask[*start] {
                            return false;
                        }
                    }

                    if let Some(end) = end {
                        if !mask[*end] {
                            return false;
                        }
                    }

                    if let (Some(start), Some(end)) = (start, end) {
                        if !mask.connactable(*start, *end) {
                            return false;
                        }
                    }
                }

                if let Some(generator) = generator {
                    if !generators.is_registered(&generator.0) {
                        return false;
                    }
                }

                if let Some(splitter) = splitter {
                    if !splitters.is_registered(&splitter.0) {
                        return false;
                    }
                }
            }
        }

        true
    }

    pub fn size(&self) -> Option<Dims3D> {
        Some(match &self.inner_spec {
            MazeSpecType::Regions { regions, .. } => regions.first()?.mask.size(),
            MazeSpecType::Simple { size, mask, .. } => {
                size.or_else(|| mask.as_ref().map(|m| m.size()))?
            }
        })
    }
}

/// Maze specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MazeSpecType {
    Regions {
        /// Specified regions.
        regions: Vec<MazeRegionSpec>,

        /// Player start position.
        start: Option<Position>,

        /// Player end position.
        end: Option<Position>,
    },
    /// Simple maze specification.
    ///
    /// Used for basic mazes, mostly specified inside the user config. User can specify only the
    /// size and the mask of the maze and used algorithms for splitting and generating the maze.
    Simple {
        /// Maze size.
        ///
        /// Can be ommited when the mask is specified.
        size: Option<Dims3D>,

        /// Player start position.
        ///
        /// We don't use [`Position`] here, because it's not possible to specify region to start in,
        /// since the regions are not generated yet.
        start: Option<Dims3D>,

        /// Player end position.
        ///
        /// We don't use [`Position`] here, because it's not possible to specify region to start in,
        /// since the regions are not generated yet.
        end: Option<Dims3D>,

        /// Mask of the maze.
        mask: Option<CellMask>,

        /// Region splitter.
        splitter: Option<Algorithm>,

        /// Region generator.
        generator: Option<Algorithm>,
    },
    // TODO: Combined, where we can specify specific regions and mask of the rest, and it's handled
    // automatically by the generator.
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Position {
    Pos(Dims3D),
    Region(u8),
}

/// Maze type.
///
/// Is not exhaustive, but generators can report that they don't support the given maze type.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MazeType {
    #[default]
    Normal,
    Tower,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MazeRegionSpec {
    pub mask: CellMask,
    pub region_type: MazeRegionType,
}

impl MazeRegionSpec {
    pub fn validate(&self, maze_size: Dims3D, generators: &GeneratorRegistry) -> bool {
        if self.mask.size() != maze_size {
            return false;
        }

        if !self.mask.is_connected() {
            return false;
        }

        match &self.region_type {
            MazeRegionType::Predefined { board } => {
                if board.size() != self.mask.size() || board.size() != maze_size {
                    return false;
                }
            }
            MazeRegionType::Generated {
                generator: (gen, _),
                seed: _,
            } => {
                if !generators.is_registered(gen) {
                    return false;
                }
            }
        }

        true
    }
}

/// Specification of a maze region.
///
/// Used in the preset files to define the regions of the maze. Mods will also be able to use this,
/// once they are implemented.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MazeRegionType {
    /// Predefined maze region.
    ///
    /// Basically already generated maze region. It is used if the user wants specific part of the
    /// maze or if it's pregenerated externally.
    Predefined {
        /// Maze of this region.
        board: MazeBoard,
    },

    /// Generated maze region.
    ///
    /// This defines how a region of the maze should be generated.
    Generated {
        /// Name of the generator used to generate the maze.
        /// Must be registered in the generator registry.
        generator: Algorithm,

        /// Optional seed for the region.
        /// If not specified, it will be calculated from the maze seed.
        seed: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "CellMaskSerde", into = "CellMaskSerde")]
pub struct CellMask(Array3D<bool>);

impl CellMask {
    pub fn new(width: usize, height: usize, depth: usize) -> Self {
        Self(Array3D::new(true, width, height, depth))
    }

    pub fn new_dims(size: Dims3D) -> Option<Self> {
        Some(Self(Array3D::new_dims(true, size)?))
    }

    pub fn new_dims_empty(size: Dims3D) -> Option<Self> {
        Some(Self(Array3D::new_dims(false, size)?))
    }

    pub fn new_2d(width: usize, height: usize) -> Self {
        Self::new(width, height, 1)
    }

    pub fn size(&self) -> Dims3D {
        self.0.size()
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&b| !b)
    }

    pub fn is_full(&self) -> bool {
        self.0.iter().all(|&b| b)
    }

    pub fn enabled_count(&self) -> usize {
        self.0.iter().filter(|&&b| b).count()
    }

    pub fn random_cell(&self, rng: &mut Random) -> Option<Dims3D> {
        // If less then 10% of the cells are enabled, we can collect all of them and choose one,
        // otherwise we can just choose random cell and check that it's enabled.

        let enabled = self.enabled_count();
        if enabled < self.0.len() / 10 {
            let enabled = self
                .0
                .iter_pos()
                .filter(|&pos| self[pos])
                .collect::<Vec<_>>();
            enabled.choose(rng).copied()
        } else if enabled > 0 {
            loop {
                let size = self.size();
                let pos = Dims3D(
                    rng.gen_range(0..size.0),
                    rng.gen_range(0..size.1),
                    rng.gen_range(0..size.2),
                );

                if self[pos] {
                    return Some(pos);
                }
            }
        } else {
            None
        }
    }

    fn dfs(&mut self, pos: Dims3D) {
        let Dims3D(width, height, depth) = self.size();

        if (pos.0 < 0 || pos.0 >= width)
            || (pos.1 < 0 || pos.1 >= height)
            || (pos.2 < 0 || pos.2 >= depth)
        {
            return;
        }

        if self[pos] {
            self[pos] = false;

            for dir in CellWall::get_in_order() {
                self.dfs(pos + dir.to_coord());
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        let mut mask = self.clone();

        if mask.is_empty() {
            return false;
        }

        for pos in Dims3D::iter_fill(Dims3D::ZERO, self.size()) {
            if mask[pos] {
                mask.dfs(pos);
                break;
            }
        }

        mask.is_empty()
    }

    pub fn connactable(&self, from: Dims3D, to: Dims3D) -> bool {
        if !self[from] || !self[to] {
            return false;
        }

        if from == to {
            return true;
        }

        let mut mask = self.clone();
        mask.dfs(from);

        !mask[to]
    }

    pub fn to_array3d(self) -> Array3D<bool> {
        self.0
    }

    pub fn as_array3d(&self) -> &Array3D<bool> {
        &self.0
    }

    pub fn fill(&mut self, value: bool) {
        self.0.fill(value);
    }

    pub fn iter_enabled(&self) -> impl Iterator<Item = Dims3D> + '_ {
        self.0.iter_pos().filter(move |&pos| self[pos])
    }
}

impl CellMask {
    pub fn combine(&self, other: &Self, op: impl Fn(bool, bool) -> bool) -> Self {
        assert!(self.size() == other.size(),);

        let (w, h, d) = self.size().into();
        Self(Array3D::from_buf(
            self.0
                .to_slice()
                .iter()
                .zip(other.0.to_slice().iter())
                .map(|(a, b)| op(*a, *b))
                .collect(),
            w as usize,
            h as usize,
            d as usize,
        ))
    }

    pub fn or(&self, other: &Self) -> Self {
        self.combine(other, |a, b| a || b)
    }

    pub fn and(&self, other: &Self) -> Self {
        self.combine(other, |a, b| a && b)
    }

    pub fn xor(&self, other: &Self) -> Self {
        self.combine(other, |a, b| a ^ b)
    }

    pub fn not(&self) -> Self {
        let (w, h, d) = self.size().into();
        Self(Array3D::from_buf(
            self.0.to_slice().iter().map(|&b| !b).collect(),
            w as usize,
            h as usize,
            d as usize,
        ))
    }
}

impl<T: Clone> Array3D<Option<T>> {
    pub fn to_mask(self) -> CellMask {
        CellMask(self.map(|v| v.is_some()))
    }
}

impl ops::Index<Dims3D> for CellMask {
    type Output = bool;

    /// Returns the value at the given index, or `false` if the index is out of bounds.
    fn index(&self, index: Dims3D) -> &Self::Output {
        self.0.get(index).unwrap_or(&false)
    }
}

impl ops::IndexMut<Dims3D> for CellMask {
    fn index_mut(&mut self, index: Dims3D) -> &mut Self::Output {
        self.0
            .get_mut(index)
            .unwrap_or_else(|| panic!("Index out of bounds: {:?}", index))
    }
}

impl From<Array3D<bool>> for CellMask {
    fn from(array: Array3D<bool>) -> Self {
        Self(array)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum CellMaskSerde {
    Bool(Array3D<bool>),
    Int(Array3D<u8>),
    Base64 { base64: String, size: Dims3D },
}

#[derive(Debug, Error)]
pub enum CellMaskSerdeError {
    #[error("Invalid base64 data: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Invalid size: {0}")]
    InvalidSize(usize),
}

impl TryFrom<CellMaskSerde> for CellMask {
    type Error = CellMaskSerdeError;

    fn try_from(mask: CellMaskSerde) -> Result<Self, Self::Error> {
        match mask {
            CellMaskSerde::Bool(array) => Ok(CellMask(array)),
            CellMaskSerde::Int(array) => Ok(CellMask(array.map(|v| v != 0))),
            CellMaskSerde::Base64 {
                base64: bytes,
                size,
            } => {
                use base64::prelude::*;
                let bits = base64::prelude::BASE64_STANDARD.decode(bytes)?;

                if (size.product() as usize).div_ceil(8) != bits.len() {
                    return Err(CellMaskSerdeError::InvalidSize(bits.len()));
                }

                // bit array -> byte array
                let mut bools = vec![false; size.product() as usize];
                for pos in Dims3D::iter_fill(Dims3D::ZERO, size) {
                    let index = pos.linear_index(size);
                    let byte = bits[index / 8];
                    let bit = (byte >> (index % 8)) & 1;
                    bools[index] = bit == 0; // for unknown fucking reason, the bits are inverted
                }

                Ok(CellMask(Array3D::from_buf(
                    bools,
                    size.0 as usize,
                    size.1 as usize,
                    size.2 as usize,
                )))
            }
        }
    }
}

impl From<CellMask> for CellMaskSerde {
    fn from(mask: CellMask) -> Self {
        CellMaskSerde::Bool(mask.0)
    }
}
