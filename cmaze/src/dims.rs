use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dims(pub i32, pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dims3D(pub i32, pub i32, pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DimsU(pub usize, pub usize);

impl Add for Dims {
    type Output = Dims;

    fn add(self, other: Dims) -> Dims {
        Dims(self.0 + other.0, self.1 + other.1)
    }
}

impl Sub for Dims {
    type Output = Dims;

    fn sub(self, other: Dims) -> Dims {
        Dims(self.0 - other.0, self.1 - other.1)
    }
}

impl AddAssign for Dims {
    fn add_assign(&mut self, other: Dims) {
        self.0 += other.0;
        self.1 += other.1;
    }
}

impl SubAssign for Dims {
    fn sub_assign(&mut self, other: Dims) {
        self.0 -= other.0;
        self.1 -= other.1;
    }
}

impl Mul<i32> for Dims {
    type Output = Dims;

    fn mul(self, other: i32) -> Dims {
        Dims(self.0 * other, self.1 * other)
    }
}

impl MulAssign<i32> for Dims {
    fn mul_assign(&mut self, other: i32) {
        self.0 *= other;
        self.1 *= other;
    }
}

impl Div<i32> for Dims {
    type Output = Dims;

    fn div(self, other: i32) -> Dims {
        Dims(self.0 / other, self.1 / other)
    }
}

impl From<(u16, u16)> for Dims {
    fn from(tuple: (u16, u16)) -> Self {
        Dims(tuple.0 as i32, tuple.1 as i32)
    }
}

impl From<Dims> for (u16, u16) {
    fn from(val: Dims) -> Self {
        (val.0 as u16, val.1 as u16)
    }
}

impl DivAssign<i32> for Dims {
    fn div_assign(&mut self, other: i32) {
        self.0 /= other;
        self.1 /= other;
    }
}

impl From<(i32, i32)> for Dims {
    fn from(tuple: (i32, i32)) -> Self {
        Dims(tuple.0, tuple.1)
    }
}

impl From<Dims> for (i32, i32) {
    fn from(val: Dims) -> Self {
        (val.0, val.1)
    }
}

impl From<Dims3D> for Dims {
    fn from(dims: Dims3D) -> Self {
        Dims(dims.0, dims.1)
    }
}

impl Dims3D {
    pub const ZERO: Dims3D = Dims3D(0, 0, 0);
    pub const ONE: Dims3D = Dims3D(1, 1, 1);

    pub fn iter_fill(from: Dims3D, to: Dims3D) -> impl Iterator<Item = Dims3D> {
        (from.0..to.0).flat_map(move |x| {
            (from.1..to.1).flat_map(move |y| (from.2..to.2).map(move |z| Dims3D(x, y, z)))
        })
    }

    pub fn all_positive(self) -> bool {
        self.0 > 0 && self.1 > 0 && self.2 > 0
    }

    pub fn all_non_negative(self) -> bool {
        self.0 >= 0 && self.1 >= 0 && self.2 >= 0
    }

    pub fn product(self) -> i32 {
        self.0 * self.1 * self.2
    }

    pub fn linear_index(&self, size: Dims3D) -> usize {
        assert!(self.all_non_negative());
        (self.2 * size.0 * size.1 + self.1 * size.0 + self.0) as usize
    }
}

impl Add for Dims3D {
    type Output = Dims3D;

    fn add(self, other: Dims3D) -> Dims3D {
        Dims3D(self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}

impl Sub for Dims3D {
    type Output = Dims3D;

    fn sub(self, other: Dims3D) -> Dims3D {
        Dims3D(self.0 - other.0, self.1 - other.1, self.2 - other.2)
    }
}

impl AddAssign for Dims3D {
    fn add_assign(&mut self, other: Dims3D) {
        self.0 += other.0;
        self.1 += other.1;
        self.2 += other.2;
    }
}

impl SubAssign for Dims3D {
    fn sub_assign(&mut self, other: Dims3D) {
        self.0 -= other.0;
        self.1 -= other.1;
        self.2 -= other.2;
    }
}

impl Mul<i32> for Dims3D {
    type Output = Dims3D;

    fn mul(self, other: i32) -> Dims3D {
        Dims3D(self.0 * other, self.1 * other, self.2 * other)
    }
}

impl Mul<f32> for Dims3D {
    type Output = Dims3D;

    fn mul(self, other: f32) -> Dims3D {
        Dims3D(
            (self.0 as f32 * other).round() as i32,
            (self.1 as f32 * other).round() as i32,
            (self.2 as f32 * other).round() as i32,
        )
    }
}

impl MulAssign<i32> for Dims3D {
    fn mul_assign(&mut self, other: i32) {
        self.0 *= other;
        self.1 *= other;
        self.2 *= other;
    }
}

impl Div<i32> for Dims3D {
    type Output = Dims3D;

    fn div(self, other: i32) -> Dims3D {
        Dims3D(self.0 / other, self.1 / other, self.2 / other)
    }
}

impl DivAssign<i32> for Dims3D {
    fn div_assign(&mut self, other: i32) {
        self.0 /= other;
        self.1 /= other;
        self.2 /= other;
    }
}

impl From<(i32, i32, i32)> for Dims3D {
    fn from(tuple: (i32, i32, i32)) -> Self {
        Dims3D(tuple.0, tuple.1, tuple.2)
    }
}

impl From<Dims3D> for (i32, i32, i32) {
    fn from(val: Dims3D) -> Self {
        (val.0, val.1, val.2)
    }
}

impl From<Dims> for Dims3D {
    fn from(dims: Dims) -> Self {
        Dims3D(dims.0, dims.1, 0)
    }
}

impl Add for DimsU {
    type Output = DimsU;

    fn add(self, other: DimsU) -> DimsU {
        DimsU(self.0 + other.0, self.1 + other.1)
    }
}

impl Sub for DimsU {
    type Output = DimsU;

    fn sub(self, other: DimsU) -> DimsU {
        DimsU(self.0 - other.0, self.1 - other.1)
    }
}

impl AddAssign for DimsU {
    fn add_assign(&mut self, other: DimsU) {
        self.0 += other.0;
        self.1 += other.1;
    }
}

impl SubAssign for DimsU {
    fn sub_assign(&mut self, other: DimsU) {
        self.0 -= other.0;
        self.1 -= other.1;
    }
}

impl Mul<usize> for DimsU {
    type Output = DimsU;

    fn mul(self, other: usize) -> DimsU {
        DimsU(self.0 * other, self.1 * other)
    }
}

impl MulAssign<usize> for DimsU {
    fn mul_assign(&mut self, other: usize) {
        self.0 *= other;
        self.1 *= other;
    }
}

impl Div<usize> for DimsU {
    type Output = DimsU;

    fn div(self, other: usize) -> DimsU {
        DimsU(self.0 / other, self.1 / other)
    }
}

impl DivAssign<usize> for DimsU {
    fn div_assign(&mut self, other: usize) {
        self.0 /= other;
        self.1 /= other;
    }
}

impl From<(usize, usize)> for DimsU {
    fn from(tuple: (usize, usize)) -> Self {
        DimsU(tuple.0, tuple.1)
    }
}

impl From<DimsU> for (usize, usize) {
    fn from(val: DimsU) -> Self {
        (val.0, val.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Offset {
    Abs(i32),
    Rel(f32),
}

impl Offset {
    pub fn to_abs(self, size: i32) -> i32 {
        match self {
            Offset::Rel(ratio) => (size as f32 * ratio).round() as i32,
            Offset::Abs(chars) => chars,
        }
    }
}

impl Default for Offset {
    fn default() -> Self {
        Offset::Rel(0.25)
    }
}
