use crate::guid::GuidExt;
use crate::object::InfoBand;
use crate::refcount;
use std::ffi::c_void;
use std::ptr;
use windows::core::{implement, ComInterface, Error, IUnknown, GUID};
use windows::Win32::Foundation::{BOOL, CLASS_E_NOAGGREGATION};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};

#[implement(IClassFactory)]
pub struct InfobandClassFactory {
    _priv: (),
}

impl InfobandClassFactory {
    pub const CLSID: GUID = InfoBand::CLSID;
}

impl InfobandClassFactory {
    pub fn new() -> Self {
        refcount::increment();
        Self { _priv: () }
    }
}

impl Drop for InfobandClassFactory {
    fn drop(&mut self) {
        refcount::decrement();
    }
}

impl IClassFactory_Impl for InfobandClassFactory {
    fn CreateInstance(
        &self,
        punkouter: Option<&IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        // SAFETY: Windows guarantees that this pointer is valid
        unsafe {
            // Must set out parameter to NULL on failure
            *ppvobject = ptr::null_mut();
        }

        // SAFETY: Windows guarantees that this pointer is valid
        let riid = unsafe { *riid };

        // Aggregation not supported
        if punkouter.is_some() {
            log::warn!("Attempted to use aggregation");
            return Err(Error::from(CLASS_E_NOAGGREGATION));
        }

        let object = IUnknown::from(InfoBand::new());

        // SAFETY: propagates same safety requirements as caller
        let result = unsafe { object.query(&riid, ppvobject.cast()).ok() };

        if let Err(e) = &result {
            log::warn!(
                "Attempted to query for unimplemented class={}, error: {:?}",
                riid.display(),
                e
            );
        }

        result
    }

    fn LockServer(&self, flock: BOOL) -> windows::core::Result<()> {
        if flock.as_bool() {
            refcount::increment();
        } else {
            refcount::decrement();
        }
        Ok(())
    }
}
