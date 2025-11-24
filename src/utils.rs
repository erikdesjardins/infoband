use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{POINT, RECT, SIZE};

pub trait RectExt {
    fn from_size(size: SIZE) -> Self;

    fn top_left_corner(self) -> POINT;
    fn size(self) -> SIZE;

    fn with_right_edge_at(self, x: i32) -> Self;
    fn with_left_edge_at(self, x: i32) -> Self;
    fn with_horizontal_midpoint_at(self, y: i32) -> Self;
    fn with_vertical_midpoint_at(self, y: i32) -> Self;

    fn width(&self) -> i32;
    fn height(&self) -> i32;
}

impl RectExt for RECT {
    fn from_size(size: SIZE) -> Self {
        Self {
            top: 0,
            left: 0,
            right: size.cx,
            bottom: size.cy,
        }
    }

    fn top_left_corner(self) -> POINT {
        POINT {
            x: self.left,
            y: self.top,
        }
    }

    fn size(self) -> SIZE {
        SIZE {
            cx: self.width(),
            cy: self.height(),
        }
    }

    fn with_left_edge_at(self, x: i32) -> Self {
        Self {
            left: x,
            right: x + self.width(),
            ..self
        }
    }

    fn with_right_edge_at(self, x: i32) -> Self {
        Self {
            left: x - self.width(),
            right: x,
            ..self
        }
    }

    fn with_horizontal_midpoint_at(self, x: i32) -> Self {
        let extra = self.width() % 2;
        let half_width = self.width() / 2;
        Self {
            left: x - half_width,
            right: x + half_width + extra,
            ..self
        }
    }

    fn with_vertical_midpoint_at(self, y: i32) -> Self {
        let extra = self.height() % 2;
        let half_height = self.height() / 2;
        Self {
            top: y - half_height,
            bottom: y + half_height + extra,
            ..self
        }
    }

    fn width(&self) -> i32 {
        self.right - self.left
    }

    fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

#[derive(Copy, Clone)]
pub struct ScalingFactor(u32);

impl ScalingFactor {
    pub const ONE: Self = Self(u16::MAX as u32 + 1);

    /// Construct a scaling factor from a fraction.
    #[track_caller]
    pub fn from_ratio(num: u32, denom: u32) -> Self {
        let factor = (u64::from(Self::ONE.0) * u64::from(num)) / u64::from(denom);
        match factor.try_into() {
            Ok(f) => Self(f),
            Err(e) => panic!("Scaling factor {num} / {denom} is too large: {e}"),
        }
    }
}

/// Fixed point scaling.
pub trait ScaleBy {
    fn scale_by(self, by: ScalingFactor) -> Self;
}

macro_rules! impl_scaleby {
    ($this:ty, via: $intermediate:ty) => {
        impl ScaleBy for $this {
            fn scale_by(self, by: ScalingFactor) -> Self {
                ((<$intermediate>::from(self) * <$intermediate>::from(by.0))
                    / <$intermediate>::from(ScalingFactor::ONE.0)) as $this
            }
        }
    };
}

impl_scaleby!(i16, via: i64);
impl_scaleby!(i32, via: i64);
impl_scaleby!(u16, via: u64);
impl_scaleby!(u32, via: u64);

impl ScaleBy for RECT {
    fn scale_by(self, by: ScalingFactor) -> Self {
        Self {
            left: self.left.scale_by(by),
            top: self.top.scale_by(by),
            right: self.right.scale_by(by),
            bottom: self.bottom.scale_by(by),
        }
    }
}

impl ScaleBy for SIZE {
    fn scale_by(self, by: ScalingFactor) -> Self {
        Self {
            cx: self.cx.scale_by(by),
            cy: self.cy.scale_by(by),
        }
    }
}

/// Represents an unscaled constant value.
/// To prevent misuse, the inner value is not vailable unless you call `scale_by` or `into_inner`.
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Unscaled<T>(T)
where
    T: ScaleBy;

impl<T> Unscaled<T>
where
    T: ScaleBy,
{
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    pub fn scale_by(self, by: ScalingFactor) -> T {
        self.0.scale_by(by)
    }

    pub const fn as_inner(&self) -> &T {
        &self.0
    }
}

impl Display for Unscaled<i32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_right_edge_at() {
        let before = RECT {
            left: 1,
            top: 2,
            right: 11,
            bottom: 12,
        };
        let after = before.with_right_edge_at(20);
        assert_eq!(after.right, 20);
        assert_eq!(after.top, before.top);
        assert_eq!(after.size(), before.size());
    }

    #[test]
    fn with_horizontal_midpoint_at() {
        let before = RECT {
            left: 1,
            top: 2,
            right: 11,
            bottom: 12,
        };
        let after = before.with_horizontal_midpoint_at(20);
        assert_eq!(after.left, 15);
        assert_eq!(after.top, before.top);
        assert_eq!(after.size(), before.size());
    }

    #[test]
    fn with_horizontal_midpoint_at_odd_width() {
        let before = RECT {
            left: 1,
            top: 2,
            right: 10,
            bottom: 10,
        };
        let after = before.with_horizontal_midpoint_at(20);
        assert_eq!(after.left, 16);
        assert_eq!(after.top, before.top);
        assert_eq!(after.size(), before.size());
    }

    #[test]
    fn with_vertical_midpoint_at() {
        let before = RECT {
            left: 1,
            top: 2,
            right: 11,
            bottom: 12,
        };
        let after = before.with_vertical_midpoint_at(20);
        assert_eq!(after.top, 15);
        assert_eq!(after.left, before.left);
        assert_eq!(after.size(), before.size());
    }

    #[test]
    fn with_vertical_midpoint_at_odd_height() {
        let before = RECT {
            left: 2,
            top: 1,
            right: 10,
            bottom: 10,
        };
        let after = before.with_vertical_midpoint_at(20);
        assert_eq!(after.top, 16);
        assert_eq!(after.left, before.left);
        assert_eq!(after.size(), before.size());
    }

    #[test]
    fn scaling_by_zero() {
        assert_eq!(0.scale_by(ScalingFactor::from_ratio(0, 1)), 0);
        assert_eq!(1.scale_by(ScalingFactor::from_ratio(0, 123)), 0);
        assert_eq!(u16::MAX.scale_by(ScalingFactor::from_ratio(0, 123)), 0);
        assert_eq!(u32::MAX.scale_by(ScalingFactor::from_ratio(0, 123)), 0);
    }

    #[test]
    fn scaling_by_one() {
        assert_eq!(0.scale_by(ScalingFactor::ONE), 0);
        assert_eq!(1.scale_by(ScalingFactor::ONE), 1);
        assert_eq!(u16::MAX.scale_by(ScalingFactor::ONE), u16::MAX);
        assert_eq!(
            u32::MAX.scale_by(ScalingFactor::from_ratio(123, 123)),
            u32::MAX
        );
        assert_eq!(123.scale_by(ScalingFactor::from_ratio(123, 123)), 123);
    }

    #[test]
    fn scaling_by_ten() {
        assert_eq!(0.scale_by(ScalingFactor::from_ratio(10, 1)), 0);
        assert_eq!(1.scale_by(ScalingFactor::from_ratio(10, 1)), 10);
        assert_eq!(
            u32::from(u16::MAX).scale_by(ScalingFactor::from_ratio(10, 1)),
            655350
        );
        assert_eq!(123.scale_by(ScalingFactor::from_ratio(100, 10)), 1230);
    }

    #[test]
    fn scaling_by_one_point_five() {
        assert_eq!(0.scale_by(ScalingFactor::from_ratio(144, 96)), 0);
        assert_eq!(1.scale_by(ScalingFactor::from_ratio(144, 96)), 1);
        assert_eq!(2.scale_by(ScalingFactor::from_ratio(144, 96)), 3);
        assert_eq!(100.scale_by(ScalingFactor::from_ratio(144, 96)), 150);
    }
}
