#![allow(non_snake_case)]
#![deny(unsafe_op_in_unsafe_fn)]

use crate::guid::GuidExt;
use factory::InfobandClassFactory;
use std::ffi::{c_void, OsStr};
use std::mem::{transmute, MaybeUninit};
use std::panic::AssertUnwindSafe;
use std::ptr;
use windows::core::{ComInterface, Result, GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    BOOL, CLASS_E_CLASSNOTAVAILABLE, E_UNEXPECTED, HMODULE, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    CoCreateInstance, ICatRegister, IClassFactory, CLSCTX_INPROC_SERVER,
};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::UI::Shell::CATID_DeskBand;

mod factory;
mod guid;
mod logging;
mod module;
mod object;
mod panic;
mod refcount;
mod registry;
mod string;
mod window;

#[no_mangle]
unsafe extern "system" fn DllMain(
    dll_module: HMODULE,
    call_reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    let result = panic::handle_unwind(|| {
        match call_reason {
            DLL_PROCESS_ATTACH => {
                // SAFETY: dll_module is guaranteed to be valid since it's passed into DllMain
                unsafe {
                    logging::init(dll_module);
                }

                // SAFETY: dll_module is guaranteed to be valid since it's passed into DllMain
                unsafe {
                    // Remove notifications on thread attach (supposed memory usage optimization)
                    DisableThreadLibraryCalls(dll_module);
                }
            }
            DLL_PROCESS_DETACH => {}
            _ => {}
        }
    });
    match result {
        Ok(()) => BOOL::from(true),
        Err(_) => BOOL::from(false),
    }
}

#[no_mangle]
unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    panic::handle_unwind(|| if refcount::is_zero() { S_OK } else { S_FALSE }).unwrap_or(S_FALSE)
}

#[no_mangle]
unsafe extern "system" fn DllGetClassObject(
    rclsid: &GUID,
    riid: &GUID,
    ppv: &mut MaybeUninit<*mut c_void>,
) -> HRESULT {
    panic::handle_unwind(AssertUnwindSafe(|| {
        // Must set out parameter to NULL on failure
        *ppv = MaybeUninit::new(ptr::null_mut());

        if *rclsid != InfobandClassFactory::CLSID {
            log::warn!("Unexpected CLSID: {}", rclsid.display());
            return CLASS_E_CLASSNOTAVAILABLE;
        }

        if *riid != IClassFactory::IID {
            log::warn!("Unexpected IID: {}", riid.display());
            return E_UNEXPECTED;
        }

        let factory = IClassFactory::from(InfobandClassFactory::new());

        // SAFETY: this is what the official example code does
        // https://github.com/microsoft/windows-rs/blob/f0c7edaf31a1bb43ef5fe4c3422bbd5c9835a341/crates/tests/component/src/lib.rs#L82
        *ppv = unsafe { transmute(factory) };

        S_OK
    }))
    .unwrap_or(E_UNEXPECTED)
}

const CLSID_STD_COMPONENT_CATEGORIES_MGR: GUID =
    GUID::from_u128(0x0002E00500000000C000000000000046);

#[no_mangle]
unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    let result = panic::handle_unwind(|| -> Result<()> {
        log::info!("Starting to register COM server...");

        let current_dll_path = module::get_current_dll_path();

        // Register COM class
        {
            let subkey = format!("CLSID\\{}", InfobandClassFactory::CLSID.display());
            let value = "InfoBand";

            log::info!("Creating registry key (subkey={}, value={})", subkey, value);

            let key = registry::Key::create(&subkey)?;

            key.set_value(None, OsStr::new(value))?;
        }

        // Register COM server and set threading model
        {
            let subkey = format!(
                "CLSID\\{}\\InprocServer32",
                InfobandClassFactory::CLSID.display()
            );
            let value = current_dll_path;

            log::info!(
                "Creating registry key (subkey={}, value={})",
                subkey,
                value.display()
            );

            let key = registry::Key::create(&subkey)?;

            key.set_value(None, value.as_os_str())?;
            key.set_value(Some("ThreadingModel"), OsStr::new("Apartment"))?;
        }

        // Register band object using COM
        {
            log::info!("Registering band object with StdComponentCategoriesMgr");

            let pcr: ICatRegister = unsafe {
                CoCreateInstance(
                    &CLSID_STD_COMPONENT_CATEGORIES_MGR,
                    None,
                    CLSCTX_INPROC_SERVER,
                )?
            };

            // SAFETY: pcr is a valid COM object
            unsafe {
                pcr.RegisterClassImplCategories(&InfobandClassFactory::CLSID, &[CATID_DeskBand])?
            };
        }

        log::info!("Completed registering COM server.");

        Ok(())
    });
    match result {
        Ok(Ok(())) => S_OK,
        Ok(Err(e)) => {
            log::error!("Failed to register COM server: {}.", e);
            e.code()
        }
        Err(_) => E_UNEXPECTED,
    }
}

#[no_mangle]
unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    let result = panic::handle_unwind(|| -> Result<()> {
        log::info!("Starting to unregister COM server...");

        // Unregister band object using COM
        {
            log::info!("Unregistering band object with StdComponentCategoriesMgr");

            let pcr: ICatRegister = unsafe {
                CoCreateInstance(
                    &CLSID_STD_COMPONENT_CATEGORIES_MGR,
                    None,
                    CLSCTX_INPROC_SERVER,
                )?
            };

            // SAFETY: pcr is a valid COM object
            unsafe {
                pcr.UnRegisterClassImplCategories(&InfobandClassFactory::CLSID, &[CATID_DeskBand])?
            };
        }

        // Unregister COM class and server
        {
            let subkey = format!("CLSID\\{}", InfobandClassFactory::CLSID.display());

            log::info!("Deleting registry key recursively (subkey={})", subkey);

            registry::Key::delete_recursively(&subkey)?;
        }

        log::info!("Completed unregistering COM server.");

        Ok(())
    });
    match result {
        Ok(Ok(())) => S_OK,
        Ok(Err(e)) => {
            log::error!("Failed to unregister COM server: {}.", e);
            e.code()
        }
        Err(_) => E_UNEXPECTED,
    }
}

#[no_mangle]
unsafe extern "system" fn DllInstall(_binstall: BOOL, _pszcmdline: PCWSTR) -> HRESULT {
    // Do nothing, install handled by DllRegisterServer/DllUnregisterServer
    S_OK
}
