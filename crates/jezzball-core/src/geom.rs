//! Low-level geometry helpers used by balls and walls.
//!
//! The core works in "cell units": the arena is a grid of 1x1 cells and the
//! balls move in continuous `f32` coordinates (ARCHITECTURE.md §2).

use std::ops::{Add, Mul, Sub};

/// Continuous 2D vector in cell units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    /// Magnitude (Euclidean length).
    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    pub fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Unit vector. If the vector is (almost) zero it returns zero, because
    /// there is no valid direction to represent.
    pub fn normalized(self) -> Vec2 {
        let len = self.length();
        if len <= f32::EPSILON {
            Vec2::new(0.0, 0.0)
        } else {
            self * (1.0 / len)
        }
    }

    /// Rotates the vector by `angle` radians (clockwise on screen).
    pub fn rotate(self, angle: f32) -> Vec2 {
        let (sin, cos) = angle.sin_cos();
        Vec2::new(self.x * cos - self.y * sin, self.x * sin + self.y * cos)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, factor: f32) -> Vec2 {
        Vec2::new(self.x * factor, self.y * factor)
    }
}

/// Rectangle in cell units (top-left corner + size).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn to_aabb(self) -> Aabb {
        Aabb {
            min_x: self.x,
            min_y: self.y,
            max_x: self.x + self.w,
            max_y: self.y + self.h,
        }
    }

    pub fn contains_point(self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }
}

/// Axis-aligned box described by its extents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Aabb {
    /// AABB centred on `center` with half-extent `half` along each axis.
    pub fn from_center(center: Vec2, half: f32) -> Self {
        Aabb {
            min_x: center.x - half,
            min_y: center.y - half,
            max_x: center.x + half,
            max_y: center.y + half,
        }
    }

    pub fn intersects(self, other: Aabb) -> bool {
        self.min_x < other.max_x
            && other.min_x < self.max_x
            && self.min_y < other.max_y
            && other.min_y < self.max_y
    }
}
