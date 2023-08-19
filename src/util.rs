use windows::Win32::Foundation::{RECT, SIZE};

pub trait RectExt {
    fn from_size(size: SIZE) -> Self;
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

    fn width(&self) -> i32 {
        self.right - self.left
    }

    fn height(&self) -> i32 {
        self.bottom - self.top
    }
}
