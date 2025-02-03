pub mod constants;
pub mod strings;

use core::fmt;

use crossterm::event::KeyEventKind;

use cmaze::{dims::*, gameboard::maze::MazeBoard};

#[inline]
pub const fn line_center(container_start: i32, container_end: i32, item_width: i32) -> i32 {
    (container_end - container_start - item_width) / 2 + container_start
}

#[inline]
pub const fn box_center(container_start: Dims, container_end: Dims, box_dims: Dims) -> Dims {
    Dims(
        line_center(container_start.0, container_end.0 + 1, box_dims.0),
        line_center(container_start.1, container_end.1 + 1, box_dims.1),
    )
}

#[inline]
pub fn maze_render_size(maze: &MazeBoard) -> Dims {
    let msize = maze.size();
    Dims(msize.0, msize.1) * 2 + Dims(1, 1)
}

#[inline]
pub fn value_if<T: Default>(cond: bool, fun: impl FnOnce() -> T) -> T {
    if cond {
        fun()
    } else {
        T::default()
    }
}

#[inline]
pub fn value_if_else<T>(cond: bool, fun: impl FnOnce() -> T, else_fun: impl FnOnce() -> T) -> T {
    if cond {
        fun()
    } else {
        else_fun()
    }
}

pub enum LineDir {
    Empty,
    Cross,
    Horizontal,
    Vertical,
    OpenLeft,
    OpenTop,
    OpenRight,
    OpenBottom,
    ClosedLeft,
    ClosedTop,
    ClosedRight,
    ClosedBottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl LineDir {
    pub const fn from_bools(left: bool, top: bool, right: bool, bottom: bool) -> Self {
        match (left, top, right, bottom) {
            (false, false, false, false) => Self::Empty,
            (true, true, true, true) => Self::Cross,
            (true, false, true, false) => Self::Horizontal,
            (false, true, false, true) => Self::Vertical,
            (false, true, false, false) => Self::OpenTop,
            (false, false, false, true) => Self::OpenBottom,
            (true, false, false, false) => Self::OpenLeft,
            (false, false, true, false) => Self::OpenRight,
            (true, false, true, true) => Self::ClosedTop,
            (true, true, true, false) => Self::ClosedBottom,
            (false, true, true, true) => Self::ClosedLeft,
            (true, true, false, true) => Self::ClosedRight,
            (true, true, false, false) => Self::TopLeft,
            (false, true, true, false) => Self::TopRight,
            (true, false, false, true) => Self::BottomLeft,
            (false, false, true, true) => Self::BottomRight,
        }
    }

    pub const fn double(&self) -> char {
        match self {
            Self::Empty => ' ',
            Self::Cross => '╬',
            Self::Horizontal => '═',
            Self::Vertical => '║',
            Self::OpenTop | Self::OpenBottom | Self::OpenLeft | Self::OpenRight => '▪',
            Self::ClosedTop => '╦',
            Self::ClosedBottom => '╩',
            Self::ClosedLeft => '╠',
            Self::ClosedRight => '╣',
            Self::TopLeft => '╝',
            Self::TopRight => '╚',
            Self::BottomLeft => '╗',
            Self::BottomRight => '╔',
        }
    }

    pub const fn round(&self) -> char {
        match self {
            Self::Empty => ' ',
            Self::Cross => '┼',
            Self::Horizontal => '─',
            Self::Vertical => '│',
            Self::OpenTop => '╷',
            Self::OpenBottom => '╵',
            Self::OpenLeft => '╴',
            Self::OpenRight => '╶',
            Self::ClosedLeft => '├',
            Self::ClosedTop => '┬',
            Self::ClosedRight => '┤',
            Self::ClosedBottom => '┴',
            Self::TopLeft => '╯',
            Self::TopRight => '╰',
            Self::BottomLeft => '╮',
            Self::BottomRight => '╭',
        }
    }
}

pub fn maze2screen_3d(pos_on_maze: impl Into<Dims3D>) -> Dims3D {
    let pos_on_maze = pos_on_maze.into();
    Dims3D(pos_on_maze.0 * 2 + 1, pos_on_maze.1 * 2 + 1, pos_on_maze.2)
}

pub fn maze2screen(pos_on_maze: impl Into<Dims3D>) -> Dims {
    let pos_on_maze = pos_on_maze.into();
    Dims(pos_on_maze.0 * 2 + 1, pos_on_maze.1 * 2 + 1)
}

pub fn is_release(k: KeyEventKind) -> bool {
    k == KeyEventKind::Release
}

pub trait ToDebug: fmt::Debug {
    fn to_debug(&self) -> String {
        format!("{:?}", self)
    }
}

impl<T: fmt::Debug> ToDebug for T {}

#[macro_export]
macro_rules! lerp {
    (($a:expr) -> ($b:expr) at $t:expr) => {
        $a + ($b - $a) * $t
    };
}

#[inline]
pub const fn yes_no(b: bool, capitalized: bool) -> &'static str {
    match (b, capitalized) {
        (true, true) => "Yes",
        (true, false) => "yes",
        (false, true) => "No",
        (false, false) => "no",
    }
}

#[inline]
pub const fn on_off(val: bool, capitalized: bool) -> &'static str {
    match (val, capitalized) {
        (true, true) => "On",
        (true, false) => "on",
        (false, true) => "Off",
        (false, false) => "off",
    }
}

/// Returns the value if it is odd, otherwise returns the value decremented by 1.
///
/// This function is useful for ensuring that a box is always an odd number of characters wide or
/// tall. So that stuff looks centered.
#[macro_export]
macro_rules! make_odd {
    ($val:expr) => {
        if $val % 2 == 0 {
            $val - 1
        } else {
            $val
        }
    };
}

/// Returns the value if it is even, otherwise returns the value incremented by 1.
#[macro_export]
macro_rules! make_even {
    ($val:expr) => {
        if $val % 2 == 0 {
            $val
        } else {
            $val - 1
        }
    };
}
