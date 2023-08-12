use crate::refcount;
use crate::window::InfoBandWindow;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ptr;
use windows::core::{implement, ComInterface, Error, IUnknown, Result, GUID, HRESULT};
use windows::Win32::Foundation::{BOOL, E_FAIL, E_INVALIDARG, E_NOTIMPL, HWND, RECT, S_FALSE};
use windows::Win32::System::Com::{IPersistStream, IPersistStream_Impl, IPersist_Impl, IStream};
use windows::Win32::System::Ole::{
    IObjectWithSite, IObjectWithSite_Impl, IOleWindow, IOleWindow_Impl,
};
use windows::Win32::UI::Shell::{
    IDeskBand, IDeskBand2, IDeskBand2_Impl, IDeskBand_Impl, IDockingWindow_Impl,
    DBIF_VIEWMODE_FLOATING, DBIMF_NORMAL, DBIMF_VARIABLEHEIGHT, DBIM_ACTUAL, DBIM_BKCOLOR,
    DBIM_INTEGRAL, DBIM_MAXSIZE, DBIM_MINSIZE, DBIM_MODEFLAGS, DBIM_TITLE, DESKBANDINFO,
};

#[implement(IObjectWithSite, IPersistStream, IDeskBand, IDeskBand2)]
pub struct InfoBand {
    site: RefCell<Option<IUnknown>>,
    parent: Cell<Option<HWND>>,
    window: RefCell<Option<InfoBandWindow>>,
    band_id: Cell<Option<u32>>,
    composition_enabled: Cell<bool>,
}

impl InfoBand {
    pub const CLSID: GUID = GUID::from_u128(0x2c3945cd709f4830be4649e14c7a42ec);
}

impl InfoBand {
    pub fn new() -> Self {
        refcount::increment();
        Self {
            site: Default::default(),
            parent: Default::default(),
            window: Default::default(),
            band_id: Default::default(),
            composition_enabled: Cell::new(false),
        }
    }
}

impl Drop for InfoBand {
    fn drop(&mut self) {
        refcount::decrement();
    }
}

impl IObjectWithSite_Impl for InfoBand {
    fn SetSite(&self, punksite: Option<&IUnknown>) -> Result<()> {
        self.site.replace(punksite.cloned());

        match punksite {
            None => {
                self.window.replace(None);
                self.parent.set(None);
                Ok(())
            }
            Some(site) => {
                let site: IOleWindow = site.cast()?;

                // SAFETY: I don't think this has any preconditions?
                let parent = unsafe { site.GetWindow()? };

                // SAFETY: window handle is guaranteed to be valid by Windows
                let window = unsafe { InfoBandWindow::new(parent)? };

                self.window.replace(Some(window));
                self.parent.set(Some(parent));

                Ok(())
            }
        }
    }

    fn GetSite(&self, riid: *const GUID, ppvsite: *mut *mut c_void) -> Result<()> {
        // SAFETY: Windows guarantees that this pointer is valid
        unsafe {
            // Must set out parameter to NULL on failure
            *ppvsite = ptr::null_mut();
        }

        match &*self.site.borrow() {
            Some(site) => {
                // SAFETY: same safety requirements as caller
                unsafe { site.query(&*riid, ppvsite.cast()).ok() }
            }
            None => Err(Error::from(E_FAIL)),
        }
    }
}

impl IPersist_Impl for InfoBand {
    fn GetClassID(&self) -> Result<GUID> {
        Ok(Self::CLSID)
    }
}

impl IPersistStream_Impl for InfoBand {
    fn IsDirty(&self) -> HRESULT {
        S_FALSE
    }

    fn Load(&self, _pstm: Option<&IStream>) -> Result<()> {
        Ok(())
    }

    fn Save(&self, _pstm: Option<&IStream>, _fcleardirty: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetSizeMax(&self) -> Result<u64> {
        Ok(0)
    }
}

impl IOleWindow_Impl for InfoBand {
    fn GetWindow(&self) -> Result<HWND> {
        match &*self.window.borrow() {
            Some(window) => Ok(window.handle()),
            None => Err(Error::from(E_FAIL)),
        }
    }

    fn ContextSensitiveHelp(&self, _fentermode: BOOL) -> Result<()> {
        Err(Error::from(E_NOTIMPL))
    }
}

impl IDockingWindow_Impl for InfoBand {
    fn ShowDW(&self, fshow: BOOL) -> Result<()> {
        if let Some(window) = &*self.window.borrow() {
            if fshow.as_bool() {
                window.show()
            } else {
                window.hide()
            }
        }
        Ok(())
    }

    fn CloseDW(&self, _dwreserved: u32) -> Result<()> {
        if let Some(window) = &*self.window.borrow() {
            window.hide();
        }
        // window destroyed by destructor
        self.window.replace(None);
        Ok(())
    }

    fn ResizeBorderDW(
        &self,
        _prcborder: *const RECT,
        _punktoolbarsite: Option<&IUnknown>,
        _freserved: BOOL,
    ) -> Result<()> {
        Err(Error::from(E_NOTIMPL))
    }
}

impl IDeskBand_Impl for InfoBand {
    fn GetBandInfo(&self, dwbandid: u32, dwviewmode: u32, pdbi: *mut DESKBANDINFO) -> Result<()> {
        if pdbi.is_null() {
            return Err(Error::from(E_INVALIDARG));
        }

        // SAFETY: Windows guarantees this is either null or valid
        let pdbi = unsafe { &mut *pdbi };

        self.band_id.set(Some(dwbandid));

        if pdbi.dwMask & DBIM_MINSIZE != 0 || pdbi.dwMask & DBIM_ACTUAL != 0 {
            if let Some(window) = &*self.window.borrow() {
                let size = window.compute_size();
                pdbi.ptMinSize = size;
                pdbi.ptActual = size;
            } else {
                log::warn!("Window size requested before window created");
            }
        }
        if pdbi.dwMask & DBIM_MAXSIZE != 0 {
            // no max height
            // TODO: without variable height, is this needed?
            pdbi.ptMaxSize.y = -1;
        }
        if pdbi.dwMask & DBIM_INTEGRAL != 0 {
            pdbi.ptIntegral.y = 1;
        }
        if pdbi.dwMask & DBIM_TITLE != 0 {
            if dwviewmode == DBIF_VIEWMODE_FLOATING {
                pdbi.wszTitle
                    .iter_mut()
                    .zip("InfoBand\0".encode_utf16())
                    .for_each(|(a, s)| *a = s);
            } else {
                // hide title
                pdbi.dwMask &= !DBIM_TITLE;
            }
        }
        if pdbi.dwMask & DBIM_MODEFLAGS != 0 {
            // TODO: do we really want variable height?
            pdbi.dwModeFlags = DBIMF_NORMAL | DBIMF_VARIABLEHEIGHT;
        }
        if pdbi.dwMask & DBIM_BKCOLOR != 0 {
            // use default background color, ignore crBkgnd
            pdbi.dwMask &= !DBIM_BKCOLOR;
        }

        Ok(())
    }
}

impl IDeskBand2_Impl for InfoBand {
    fn CanRenderComposited(&self) -> Result<BOOL> {
        Ok(true.into())
    }

    fn SetCompositionState(&self, fcompositionenabled: BOOL) -> Result<()> {
        let enabled = fcompositionenabled.as_bool();

        if let Some(window) = &*self.window.borrow() {
            self.composition_enabled.set(enabled);
            window.set_composition_enabled(enabled);
            window.invalidate();
            window.update();
            Ok(())
        } else {
            log::warn!(
                "Attempting to set composition state ({}) before window created",
                enabled
            );
            Err(Error::from(E_FAIL))
        }
    }

    fn GetCompositionState(&self) -> Result<BOOL> {
        Ok(self.composition_enabled.get().into())
    }
}
