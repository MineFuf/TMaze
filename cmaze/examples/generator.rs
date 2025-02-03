use std::env;
use std::hash::DefaultHasher;

use std::hash::{Hash as _, Hasher as _};

use cmaze::algorithms::region_splitter::{DefaultRegionSplitter, RegionCount, RegionSplitter as _};
use cmaze::algorithms::Params;
use cmaze::{
    algorithms::{CellMask, Random},
    array::Array3D,
    dims::Dims3D,
    progress::ProgressHandle,
};

use rand::{thread_rng, Rng as _, SeedableRng as _};

fn main() {
    let args = env::args()
        .skip(1)
        .take(4)
        .map(|s| s.parse())
        .collect::<Result<Vec<i128>, _>>()
        .expect("Expected 3 integers");

    assert!(
        args.len() == 3 || args.len() == 4,
        "Expected 3 or 4 integers"
    );

    let input_seed = args.get(3).copied().map(|seed| seed as u64);
    let seed = input_seed.unwrap_or_else(|| thread_rng().gen());
    let mut rng = Random::seed_from_u64(seed);

    if input_seed.is_none() {
        println!("Seed: {}", seed);
    }

    let base_hash = rng.gen::<u64>();

    let size = Dims3D(args[0] as i32, args[1] as i32, 1);
    let point_count = args[2] as u8;

    let progress = ProgressHandle::new();
    let mask = CellMask::new_dims(size).unwrap();
    let splitter = DefaultRegionSplitter;
    let groups = splitter.split(&mask, &mut rng, progress, &Params::default()).unwrap();

    show_array(
        &groups,
        Array3D::new_dims(true, size).unwrap(),
        base_hash,
        size,
        '•',
    );

    // NOTE: Don't remove
    // let masks = Generator::split_to_masks(point_count, &groups);
    // for (i, mask) in masks.into_iter().enumerate() {
    //     println!("Mask {}", i);
    //     show_array(&groups, mask.to_array3d(), base_hash, size, '•');
    // }
}

fn show_array(
    groups: &Array3D<u8>,
    mask: Array3D<bool>,
    base_hash: u64,
    size: Dims3D,
    empty_char: char,
) {
    for cell in mask.iter_pos() {
        let group = groups[cell];

        if !mask[cell] {
            print!("{}", empty_char);
        } else {
            let mut hasher = DefaultHasher::new();
            group.hash(&mut hasher);

            let hash = hasher.finish().wrapping_add(base_hash);
            let (r, g, b) = ((hash >> 16) as u8, (hash >> 8) as u8, hash as u8);

            print!("\x1b[48;2;{r};{g};{b}m \x1b[0m");
        }
        if cell.0 == size.0 - 1 {
            println!();
        }
    }
}
