use std::panic::{catch_unwind, UnwindSafe};

pub fn handle_unwind<T>(f: impl FnOnce() -> T + UnwindSafe) -> Result<T, ()> {
    catch_unwind(|| match catch_unwind(f) {
        Ok(x) => Ok(x),
        Err(e) => {
            log::error!("Caught panic: {:?}", e);
            Err(())
        }
    })
    .unwrap_or_else(|_| Err(()))
}
