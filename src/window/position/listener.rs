use crate::constants::UM_QUEUE_TRAY_POSITION_CHECK;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationElement, IUIAutomationStructureChangedEventHandler,
    IUIAutomationStructureChangedEventHandler_Impl, StructureChangeType, TreeScope_Subtree,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_USER};
use windows::core::{Ref, Result};
use windows_core::implement;

pub struct TrayListenerManager {
    automation: IUIAutomation,
    listener: IUIAutomationStructureChangedEventHandler,
    registered_element: IUIAutomationElement,
}

impl Drop for TrayListenerManager {
    fn drop(&mut self) {
        if let Err(e) = unsafe {
            self.automation
                .RemoveStructureChangedEventHandler(&self.registered_element, &self.listener)
        } {
            log::warn!("Unregistering tray listener failed: {e}");
        }
    }
}

impl TrayListenerManager {
    pub fn new(
        window: HWND,
        automation: IUIAutomation,
        element: IUIAutomationElement,
    ) -> Result<Self> {
        let listener = IUIAutomationStructureChangedEventHandler::from(TrayListener { window });

        register_listener(&automation, &element, &listener)?;

        let registered_element = element;

        Ok(Self {
            automation,
            listener,
            registered_element,
        })
    }

    pub fn element(&self) -> &IUIAutomationElement {
        &self.registered_element
    }

    pub fn refresh_element(&mut self, element: IUIAutomationElement) -> Result<()> {
        // Unregister old element
        if let Err(e) = unsafe {
            self.automation
                .RemoveStructureChangedEventHandler(&self.registered_element, &self.listener)
        } {
            log::warn!("Unregistering old tray listener failed: {e}");
        }

        // Register new element
        register_listener(&self.automation, &element, &self.listener)?;

        self.registered_element = element;

        Ok(())
    }
}

fn register_listener(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
    listener: &IUIAutomationStructureChangedEventHandler,
) -> Result<()> {
    unsafe {
        automation.AddStructureChangedEventHandler(element, TreeScope_Subtree, None, listener)
    }
}

#[implement(IUIAutomationStructureChangedEventHandler)]
struct TrayListener {
    window: HWND,
}

impl IUIAutomationStructureChangedEventHandler_Impl for TrayListener_Impl {
    fn HandleStructureChangedEvent(
        &self,
        _: Ref<'_, IUIAutomationElement>,
        _: StructureChangeType,
        _: *const SAFEARRAY,
    ) -> Result<()> {
        // WARNING: this may be called from another thread, so we can only do thread-safe operations here.

        // Send a message to the main thread to enqueue a tray position check.
        unsafe {
            PostMessageW(
                Some(self.window),
                WM_USER,
                UM_QUEUE_TRAY_POSITION_CHECK,
                LPARAM(0),
            )
        }
    }
}
