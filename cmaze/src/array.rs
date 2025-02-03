use std::ops;

use serde::{Deserialize, Serialize};

use crate::dims::{Dims, Dims3D};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "Array3DSerde<T>", into = "Array3DSerde<T>")]
pub struct Array3D<T: Clone> {
    buf: Vec<T>,
    width: usize,
    height: usize,
    depth: usize,
}

impl<T: Clone> Array3D<T> {
    pub fn size(&self) -> Dims3D {
        Dims3D(self.width as i32, self.height as i32, self.depth as i32)
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn dim_to_idx(&self, pos: Dims3D) -> Option<usize> {
        let Dims3D(x, y, z) = pos;
        let (x, y, z) = (x as usize, y as usize, z as usize);

        if x >= self.width || y >= self.height || z >= self.depth {
            return None;
        }

        Some(z * self.width * self.height + y * self.width + x)
    }

    pub fn idx_to_dim(&self, idx: usize) -> Option<Dims3D> {
        if idx >= self.buf.len() {
            return None;
        }

        let x = idx % self.width;
        let y = (idx / self.width) % self.height;
        let z = idx / (self.width * self.height);

        Some(Dims3D(x as i32, y as i32, z as i32))
    }

    pub fn get(&self, pos: Dims3D) -> Option<&T> {
        self.dim_to_idx(pos).and_then(|i| self.buf.get(i))
    }

    pub fn get_mut(&mut self, pos: Dims3D) -> Option<&mut T> {
        self.dim_to_idx(pos).and_then(move |i| self.buf.get_mut(i))
    }
}

impl<T: Clone> Array3D<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf.iter()
    }

    pub fn iter_pos(&self) -> impl Iterator<Item = Dims3D> + use<'_, T> {
        (0..self.buf.len()).filter_map(move |i| self.idx_to_dim(i))
    }

    pub fn all(&self, f: impl Fn(&T) -> bool) -> bool {
        self.buf.iter().all(f)
    }

    pub fn map<U: Clone>(self, f: impl Fn(T) -> U) -> Array3D<U> {
        Array3D {
            buf: self.buf.into_iter().map(f).collect(),
            width: self.width,
            height: self.height,
            depth: self.depth,
        }
    }

    pub fn layer(&self, z: usize) -> Option<Array2DView<T>> {
        if z >= self.depth {
            return None;
        }

        let z = z as i32;
        let start = self.dim_to_idx(Dims3D(0, 0, z))?;
        let end = start + self.width * self.height;

        Some(Array2DView {
            buf: &self.buf[start..end],
            width: self.width,
            height: self.height,
        })
    }

    pub fn layers(&self) -> impl Iterator<Item = Array2DView<T>> + '_ {
        (0..self.depth).map(move |z| self.layer(z).unwrap())
    }

    pub fn mask(self, mask: &Array3D<bool>) -> Option<Array3D<Option<T>>> {
        if self.size() != mask.size() {
            return None;
        }

        Some(Array3D {
            buf: self
                .buf
                .into_iter()
                .zip(mask.buf.iter())
                .map(|(item, mask)| if *mask { Some(item) } else { None })
                .collect(),
            width: self.width,
            height: self.height,
            depth: self.depth,
        })
    }
}

impl<T: Clone> Array3D<T> {
    pub fn new(item: T, width: usize, height: usize, depth: usize) -> Self {
        // Check for overflow
        assert!(width
            .checked_mul(height)
            .and_then(|v| v.checked_mul(depth))
            .is_some());

        Self {
            buf: vec![item.clone(); width * height * depth],
            width,
            height,
            depth,
        }
    }

    pub fn new_dims(item: T, size: Dims3D) -> Option<Self> {
        if !size.all_non_negative() {
            return None;
        }
        Some(Self::new(
            item,
            size.0 as usize,
            size.1 as usize,
            size.2 as usize,
        ))
    }

    pub fn fill(&mut self, item: T) {
        self.buf.fill(item);
    }

    pub fn to_buf(self) -> Vec<T> {
        self.buf
    }

    pub fn to_slice(&self) -> &[T] {
        &self.buf
    }
}

impl<T: Clone> Array3D<T> {
    pub fn from_buf(buf: Vec<T>, width: usize, height: usize, depth: usize) -> Self {
        Self {
            buf,
            width,
            height,
            depth,
        }
    }
}

impl<T: Clone> ops::Index<Dims3D> for Array3D<T> {
    type Output = T;

    fn index(&self, index: Dims3D) -> &Self::Output {
        self.dim_to_idx(index)
            .and_then(|i| self.buf.get(i))
            .expect("Index out of bounds")
    }
}

impl<T: Clone> ops::IndexMut<Dims3D> for Array3D<T> {
    fn index_mut(&mut self, index: Dims3D) -> &mut Self::Output {
        self.dim_to_idx(index)
            .and_then(|i| self.buf.get_mut(i))
            .expect("Index out of bounds")
    }
}

pub struct Array2DView<'a, T> {
    buf: &'a [T],
    width: usize,
    height: usize,
}

impl<'a, T> Array2DView<'a, T> {
    pub fn size(&self) -> Dims {
        Dims(self.width as i32, self.height as i32)
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn dim_to_idx(&self, pos: Dims) -> Option<usize> {
        let Dims(x, y) = pos;
        let (x, y) = (x as usize, y as usize);

        if x >= self.width || y >= self.height {
            return None;
        }

        Some(y * self.width + x)
    }

    pub fn idx_to_dim(&self, idx: usize) -> Option<Dims> {
        if idx >= self.buf.len() {
            return None;
        }

        let x = idx % self.width;
        let y = idx / self.width;

        Some(Dims(x as i32, y as i32))
    }

    pub fn get(&self, pos: Dims) -> Option<&T> {
        self.dim_to_idx(pos).and_then(|i| self.buf.get(i))
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf.iter()
    }

    pub fn iter_pos(&self) -> impl Iterator<Item = Dims> + '_ {
        (0..self.buf.len()).filter_map(move |i| self.idx_to_dim(i))
    }

    pub fn to_array(&self) -> Array3D<T>
    where
        T: Clone,
    {
        Array3D::from_buf(self.buf.to_vec(), self.width, self.height, 1)
    }
}

impl<T> ops::Index<Dims> for Array2DView<'_, T> {
    type Output = T;

    fn index(&self, index: Dims) -> &Self::Output {
        self.dim_to_idx(index)
            .and_then(|i| self.buf.get(i))
            .expect("Index out of bounds")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum Array3DSerde<T: Clone> {
    Flat { buf: Vec<T>, size: Dims3D },
    Dim3D(Vec<Vec<Vec<T>>>),
    Dim2D(Vec<Vec<T>>),
}

impl<T: Clone> TryFrom<Array3DSerde<T>> for Array3D<T> {
    type Error = &'static str;

    fn try_from(value: Array3DSerde<T>) -> Result<Self, Self::Error> {
        match value {
            Array3DSerde::Flat { buf, size } => {
                if !(size.all_non_negative()
                    && buf.len() == size.0 as usize * size.1 as usize * size.2 as usize)
                {
                    return Err("Size mismatch");
                }
                Ok(Array3D {
                    buf,
                    width: size.0 as usize,
                    height: size.1 as usize,
                    depth: size.2 as usize,
                })
            }

            Array3DSerde::Dim3D(buf) => {
                let size = Dims3D(
                    buf.first()
                        .and_then(|v| v.first())
                        .map(|v| v.len())
                        .unwrap_or(0) as i32,
                    buf.first().map(|v| v.len()).unwrap_or(0) as i32,
                    buf.len() as i32,
                );
                if buf.iter().any(|v| v.len() != size.1 as usize)
                    || buf
                        .iter()
                        .any(|v| v.iter().any(|v| v.len() != size.0 as usize))
                {
                    return Err("Size mismatch");
                }
                Ok(Array3D {
                    buf: buf.into_iter().flatten().flatten().collect(),
                    width: size.0 as usize,
                    height: size.1 as usize,
                    depth: size.2 as usize,
                })
            }

            Array3DSerde::Dim2D(buf) => {
                let size = Dims3D(
                    buf.first().map(|v| v.len()).unwrap_or(0) as i32,
                    buf.len() as i32,
                    1,
                );
                if buf.iter().any(|v| v.len() != size.0 as usize) {
                    return Err("Size mismatch");
                }
                Ok(Array3D {
                    buf: buf.into_iter().flatten().collect(),
                    width: size.0 as usize,
                    height: size.1 as usize,
                    depth: size.2 as usize,
                })
            }
        }
    }
}

impl<T: Clone> From<Array3D<T>> for Array3DSerde<T> {
    fn from(value: Array3D<T>) -> Self {
        Array3DSerde::Flat {
            size: value.size(),
            buf: value.buf,
        }
    }
}
