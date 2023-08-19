use windows::Win32::Foundation::{RECT, SIZE};

pub trait RectExt {
    fn from_size(size: SIZE) -> Self;

    fn with_right_edge_at(self, x: i32) -> Self;
    fn with_top_edge_at(self, x: i32) -> Self;

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

    fn with_right_edge_at(self, x: i32) -> Self {
        Self {
            left: x - self.width(),
            right: x,
            ..self
        }
    }

    fn with_top_edge_at(self, y: i32) -> Self {
        Self {
            top: y,
            bottom: y + self.height(),
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
