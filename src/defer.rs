use std::mem::ManuallyDrop;

#[macro_export]
macro_rules! defer {
    ($f:stmt ;) => {
        let _x = $crate::defer::Defer(::std::mem::ManuallyDrop::new(
            #[allow(redundant_semicolons)]
            || {
                $f;
            },
        ));
    };
}

#[doc(hidden)]
pub struct Defer<F: FnOnce()>(pub ManuallyDrop<F>);

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        let f = unsafe { ManuallyDrop::take(&mut self.0) };
        f();
    }
}
