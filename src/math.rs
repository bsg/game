use std::ops::{Add, DivAssign, Mul, MulAssign};

pub trait Scalar<T>: Add<Output = T> + Mul<T, Output = T> + MulAssign + DivAssign + Copy + Sized {
    fn zero() -> T;
    fn sqrt(self) -> T;
    fn powi(self, n: i32) -> T;
}

#[allow(dead_code)]
impl Scalar<f32> for f32 {
    fn zero() -> f32 {
        0.0
    }

    fn powi(self, n: i32) -> f32 {
        self.powi(n)
    }

    fn sqrt(self) -> f32 {
        f32::sqrt(self)
    }
}


#[allow(dead_code)]
impl Scalar<i32> for i32 {
    fn zero() -> i32 {
        0
    }

    fn powi(self, n: i32) -> i32 {
        i32::pow(self, n.try_into().unwrap())
    }

    fn sqrt(self) -> i32 {
        (self as f32).sqrt() as i32
    }
}

#[allow(dead_code)]
impl Scalar<u32> for u32 {
    fn zero() -> u32 {
        0
    }

    fn powi(self, n: i32) -> u32 {
        u32::pow(self, n.try_into().unwrap())
    }

    fn sqrt(self) -> u32 {
        (self as f32).sqrt() as u32
    }
}

#[derive(Clone, Copy)]
pub struct Vec2<T> {
    pub x: T,
    pub y: T,
}

#[allow(dead_code)]
impl<T: Scalar<T>> Vec2<T> {
    pub fn new(x: T, y: T) -> Vec2<T> {
        return Vec2 { x, y };
    }

    pub fn zero() -> Vec2<T> {
        return Vec2 {
            x: T::zero(),
            y: T::zero(),
        };
    }

    pub fn magnitude(&self) -> T {
        T::sqrt(self.x.powi(2) + self.y.powi(2))
    }

    pub fn normalize(&mut self) {
        self.x /= self.magnitude();
        self.y /= self.magnitude();
    }

    pub fn scale(&mut self, scale: T) {
        self.x *= scale;
        self.y *= scale;
    }

    pub fn scaled(&self, scale: T) -> Vec2<T> {
        Vec2::<T>::new(self.x * scale, self.y * scale)
    }
}

#[derive(Clone, Copy)]
pub struct Vec3<T> {
    pub x: T,
    pub y: T,
    pub z: T,
}

#[allow(dead_code)]
impl<T: Scalar<T>> Vec3<T> {
    pub fn new(x: T, y: T, z: T) -> Vec3<T> {
        return Vec3 { x, y, z };
    }

    pub fn zero() -> Vec3<T> {
        return Vec3 {
            x: T::zero(),
            y: T::zero(),
            z: T::zero(),
        };
    }
}
