use windows::Win32::Foundation::RECT;

pub trait RectExt {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
}

impl RectExt for RECT {
    fn width(&self) -> i32 {
        self.right - self.left
    }

    fn height(&self) -> i32 {
        self.bottom - self.top
    }
}
