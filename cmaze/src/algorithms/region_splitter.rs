use hashbrown::HashSet;
use rand::seq::SliceRandom as _;
use smallvec::SmallVec;

use std::{fmt, str::FromStr};

use crate::{array::Array3D, dims::*, gameboard::CellWall, progress::ProgressHandle};

use super::{CellMask, Params, Random};

pub trait RegionSplitter: fmt::Debug + Sync + Send {
    fn split(
        &self,
        mask: &CellMask,
        rng: &mut Random,
        progress: ProgressHandle,
        params: &Params,
    ) -> Option<Array3D<u8>>;
}

#[derive(Debug, Clone, Copy)]
pub enum RegionCount {
    Per(usize),
    Exact(u8),
}

impl FromStr for RegionCount {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.splitn(2, ':').collect::<Vec<_>>();
        match t.as_slice() {
            ["per", n] => n.parse().map(RegionCount::Per).map_err(|_| ()),
            ["exact", n] | [n] => n.parse().map(RegionCount::Exact).map_err(|_| ()),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Default)]
pub struct DefaultRegionSplitter;

impl DefaultRegionSplitter {
    pub fn random_points(mask: &CellMask, count: u8, rng: &mut Random) -> Vec<Dims3D> {
        let count = count as usize;
        let mut points = Vec::with_capacity(count);

        while points.len() < count {
            // FIXME: this is absolutely horrible and slow implementation,
            // but since we don't sample a lot of points, it should be fine. I hope...
            let point = mask.random_cell(rng).unwrap();

            if !points.contains(&point) {
                points.push(point);
            }
        }

        points
    }
    pub fn split_groups(
        points: Vec<Dims3D>,
        mask: &CellMask,
        rng: &mut Random,
        progress: ProgressHandle,
    ) -> Option<Array3D<u8>> {
        assert!(points.len() <= u8::MAX as usize);
        assert!(!points.is_empty());
        assert!(points.iter().collect::<HashSet<_>>().len() == points.len());

        progress.lock().from = mask.enabled_count();

        let size = mask.size();
        let mut groups = Array3D::new_dims(None, size).unwrap();

        // assign initial groups
        for (i, point) in points.into_iter().enumerate() {
            groups[point] = Some((i as u8, usize::MAX));
        }

        // This algorithm uses simple flood with diamond shaped search and randomized group order
        // on each cycle.
        // If it's found that it generates boring results, it can be replaced with a more complex
        // one.

        let mut cycle = 0usize;

        loop {
            if groups.all(|group| group.is_some()) {
                break;
            }

            let mut set_new = 0;
            for cell in Dims3D::iter_fill(Dims3D::ZERO, size) {
                if !mask[cell] || groups[cell].is_some() {
                    continue;
                }

                let neighbors = CellWall::get_in_order()
                    .into_iter()
                    .map(|dir| cell + dir.to_coord())
                    .filter_map(|pos| {
                        groups.get(pos).and_then(|g| {
                            g.and_then(|(g, cyc)| if cyc == cycle { None } else { Some(g) })
                        })
                    })
                    .collect::<SmallVec<[_; 6]>>();

                if let Some(new_group) = neighbors.choose(rng) {
                    groups[cell] = Some((*new_group, cycle));
                    set_new += 1;
                }
            }

            cycle = cycle.wrapping_add(1);
            progress.lock().done += set_new;
            if progress.is_stopped() {
                return None;
            }
        }

        progress.lock().finish();

        Some(groups.map(|group| group.unwrap().0))
    }
}

impl RegionSplitter for DefaultRegionSplitter {
    fn split(
        &self,
        mask: &CellMask,
        rng: &mut Random,
        progress: ProgressHandle,
        params: &Params,
    ) -> Option<Array3D<u8>> {
        let count = params.parsed_or_warn("count", RegionCount::Per(100));

        let region_count = match count {
            RegionCount::Per(every) => mask.enabled_count() / every,
            RegionCount::Exact(count) => count as usize,
        }
        .clamp(1, u8::MAX as usize) as u8;

        let points = Self::random_points(mask, region_count, rng);

        progress.lock().from = mask.enabled_count();

        let size = mask.size();
        let mut groups = Array3D::new_dims(None, size).unwrap();

        // assign initial groups
        for (i, point) in points.into_iter().enumerate() {
            groups[point] = Some((i as u8, usize::MAX));
        }

        // This algorithm uses simple flood with diamond shaped search and randomized group order
        // on each cycle.
        // If it's found that it generates boring results, it can be replaced with a more complex
        // one.

        let mut cycle = 0usize;

        loop {
            if groups.iter_pos().all(|pos| groups[pos].is_some() || !mask[pos]) {
                break;
            }

            let mut set_new = 0;
            for cell in Dims3D::iter_fill(Dims3D::ZERO, size) {
                if !mask[cell] || groups[cell].is_some() {
                    continue;
                }

                let neighbors = CellWall::get_in_order()
                    .into_iter()
                    .map(|dir| cell + dir.to_coord())
                    .filter_map(|pos| {
                        groups.get(pos).and_then(|g| {
                            g.and_then(|(g, cyc)| if cyc == cycle { None } else { Some(g) })
                        })
                    })
                    .collect::<SmallVec<[_; 6]>>();

                if let Some(new_group) = neighbors.choose(rng) {
                    groups[cell] = Some((*new_group, cycle));
                    set_new += 1;
                }
            }

            cycle = cycle.wrapping_add(1);
            progress.lock().done += set_new;
            if progress.is_stopped() {
                return None;
            }
        }

        progress.lock().finish();

        Some(groups.map(|group| group.unwrap_or_default().0))
    }
}
