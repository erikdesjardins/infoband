use crate::util::OptionExt;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Mutex, OnceLock, PoisonError};
use windows::Win32::System::DataExchange::GetClipboardFormatNameW;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

static MESSAGES: OnceLock<HashMap<u32, &str>> = OnceLock::new();

fn messages() -> &'static HashMap<u32, &'static str> {
    MESSAGES.get_or_init(
        #[cold]
        #[inline(never)]
        || {
            use windows::Win32::UI::WindowsAndMessaging::*;

            let mut map = HashMap::new();

            macro_rules! add {
                ($($msg:ident),* $(,)?) => {
                    $(
                        map.insert($msg, stringify!($msg));
                    )*
                };
            }

            add!(
                WM_ACTIVATE,
                WM_ACTIVATEAPP,
                WM_AFXFIRST,
                WM_AFXLAST,
                WM_APP,
                WM_APPCOMMAND,
                WM_ASKCBFORMATNAME,
                WM_CANCELJOURNAL,
                WM_CANCELMODE,
                WM_CAPTURECHANGED,
                WM_CHANGECBCHAIN,
                WM_CHANGEUISTATE,
                WM_CHAR,
                WM_CHARTOITEM,
                WM_CHILDACTIVATE,
                WM_CLEAR,
                WM_CLIPBOARDUPDATE,
                WM_CLOSE,
                WM_COMMAND,
                WM_COMMNOTIFY,
                WM_COMPACTING,
                WM_COMPAREITEM,
                WM_CONTEXTMENU,
                WM_COPY,
                WM_COPYDATA,
                WM_CREATE,
                WM_CTLCOLORBTN,
                WM_CTLCOLORDLG,
                WM_CTLCOLOREDIT,
                WM_CTLCOLORLISTBOX,
                WM_CTLCOLORMSGBOX,
                WM_CTLCOLORSCROLLBAR,
                WM_CTLCOLORSTATIC,
                WM_CUT,
                WM_DEADCHAR,
                WM_DELETEITEM,
                WM_DESTROY,
                WM_DESTROYCLIPBOARD,
                WM_DEVICECHANGE,
                WM_DEVMODECHANGE,
                WM_DISPLAYCHANGE,
                WM_DPICHANGED,
                WM_DPICHANGED_AFTERPARENT,
                WM_DPICHANGED_BEFOREPARENT,
                WM_DRAWCLIPBOARD,
                WM_DRAWITEM,
                WM_DROPFILES,
                WM_DWMCOLORIZATIONCOLORCHANGED,
                WM_DWMCOMPOSITIONCHANGED,
                WM_DWMNCRENDERINGCHANGED,
                WM_DWMSENDICONICLIVEPREVIEWBITMAP,
                WM_DWMSENDICONICTHUMBNAIL,
                WM_DWMWINDOWMAXIMIZEDCHANGE,
                WM_ENABLE,
                WM_ENDSESSION,
                WM_ENTERIDLE,
                WM_ENTERMENULOOP,
                WM_ENTERSIZEMOVE,
                WM_ERASEBKGND,
                WM_EXITMENULOOP,
                WM_EXITSIZEMOVE,
                WM_FONTCHANGE,
                WM_GESTURE,
                WM_GESTURENOTIFY,
                WM_GETDLGCODE,
                WM_GETDPISCALEDSIZE,
                WM_GETFONT,
                WM_GETHOTKEY,
                WM_GETICON,
                WM_GETMINMAXINFO,
                WM_GETOBJECT,
                WM_GETTEXT,
                WM_GETTEXTLENGTH,
                WM_GETTITLEBARINFOEX,
                WM_HANDHELDFIRST,
                WM_HANDHELDLAST,
                WM_HELP,
                WM_HOTKEY,
                WM_HSCROLL,
                WM_HSCROLLCLIPBOARD,
                WM_ICONERASEBKGND,
                WM_IME_CHAR,
                WM_IME_COMPOSITION,
                WM_IME_COMPOSITIONFULL,
                WM_IME_CONTROL,
                WM_IME_ENDCOMPOSITION,
                WM_IME_KEYDOWN,
                WM_IME_KEYLAST,
                WM_IME_KEYUP,
                WM_IME_NOTIFY,
                WM_IME_REQUEST,
                WM_IME_SELECT,
                WM_IME_SETCONTEXT,
                WM_IME_STARTCOMPOSITION,
                WM_INITDIALOG,
                WM_INITMENU,
                WM_INITMENUPOPUP,
                WM_INPUT,
                WM_INPUTLANGCHANGE,
                WM_INPUTLANGCHANGEREQUEST,
                WM_INPUT_DEVICE_CHANGE,
                WM_KEYDOWN,
                WM_KEYFIRST,
                WM_KEYLAST,
                WM_KEYUP,
                WM_KILLFOCUS,
                WM_LBUTTONDBLCLK,
                WM_LBUTTONDOWN,
                WM_LBUTTONUP,
                WM_MBUTTONDBLCLK,
                WM_MBUTTONDOWN,
                WM_MBUTTONUP,
                WM_MDIACTIVATE,
                WM_MDICASCADE,
                WM_MDICREATE,
                WM_MDIDESTROY,
                WM_MDIGETACTIVE,
                WM_MDIICONARRANGE,
                WM_MDIMAXIMIZE,
                WM_MDINEXT,
                WM_MDIREFRESHMENU,
                WM_MDIRESTORE,
                WM_MDISETMENU,
                WM_MDITILE,
                WM_MEASUREITEM,
                WM_MENUCHAR,
                WM_MENUCOMMAND,
                WM_MENUDRAG,
                WM_MENUGETOBJECT,
                WM_MENURBUTTONUP,
                WM_MENUSELECT,
                WM_MOUSEACTIVATE,
                WM_MOUSEFIRST,
                WM_MOUSEHWHEEL,
                WM_MOUSELAST,
                WM_MOUSEMOVE,
                WM_MOUSEWHEEL,
                WM_MOVE,
                WM_MOVING,
                WM_NCACTIVATE,
                WM_NCCALCSIZE,
                WM_NCCREATE,
                WM_NCDESTROY,
                WM_NCHITTEST,
                WM_NCLBUTTONDBLCLK,
                WM_NCLBUTTONDOWN,
                WM_NCLBUTTONUP,
                WM_NCMBUTTONDBLCLK,
                WM_NCMBUTTONDOWN,
                WM_NCMBUTTONUP,
                WM_NCMOUSEHOVER,
                WM_NCMOUSELEAVE,
                WM_NCMOUSEMOVE,
                WM_NCPAINT,
                WM_NCPOINTERDOWN,
                WM_NCPOINTERUP,
                WM_NCPOINTERUPDATE,
                WM_NCRBUTTONDBLCLK,
                WM_NCRBUTTONDOWN,
                WM_NCRBUTTONUP,
                WM_NCXBUTTONDBLCLK,
                WM_NCXBUTTONDOWN,
                WM_NCXBUTTONUP,
                WM_NEXTDLGCTL,
                WM_NEXTMENU,
                WM_NOTIFY,
                WM_NOTIFYFORMAT,
                WM_NULL,
                WM_PAINT,
                WM_PAINTCLIPBOARD,
                WM_PAINTICON,
                WM_PALETTECHANGED,
                WM_PALETTEISCHANGING,
                WM_PARENTNOTIFY,
                WM_PASTE,
                WM_PENWINFIRST,
                WM_PENWINLAST,
                WM_POINTERACTIVATE,
                WM_POINTERCAPTURECHANGED,
                WM_POINTERDEVICECHANGE,
                WM_POINTERDEVICEINRANGE,
                WM_POINTERDEVICEOUTOFRANGE,
                WM_POINTERDOWN,
                WM_POINTERENTER,
                WM_POINTERHWHEEL,
                WM_POINTERLEAVE,
                WM_POINTERROUTEDAWAY,
                WM_POINTERROUTEDRELEASED,
                WM_POINTERROUTEDTO,
                WM_POINTERUP,
                WM_POINTERUPDATE,
                WM_POINTERWHEEL,
                WM_POWER,
                WM_POWERBROADCAST,
                WM_PRINT,
                WM_PRINTCLIENT,
                WM_QUERYDRAGICON,
                WM_QUERYENDSESSION,
                WM_QUERYNEWPALETTE,
                WM_QUERYOPEN,
                WM_QUERYUISTATE,
                WM_QUEUESYNC,
                WM_QUIT,
                WM_RBUTTONDBLCLK,
                WM_RBUTTONDOWN,
                WM_RBUTTONUP,
                WM_RENDERALLFORMATS,
                WM_RENDERFORMAT,
                WM_SETCURSOR,
                WM_SETFOCUS,
                WM_SETFONT,
                WM_SETHOTKEY,
                WM_SETICON,
                WM_SETREDRAW,
                WM_SETTEXT,
                WM_SETTINGCHANGE,
                WM_SHOWWINDOW,
                WM_SIZE,
                WM_SIZECLIPBOARD,
                WM_SIZING,
                WM_SPOOLERSTATUS,
                WM_STYLECHANGED,
                WM_STYLECHANGING,
                WM_SYNCPAINT,
                WM_SYSCHAR,
                WM_SYSCOLORCHANGE,
                WM_SYSCOMMAND,
                WM_SYSDEADCHAR,
                WM_SYSKEYDOWN,
                WM_SYSKEYUP,
                WM_TABLET_FIRST,
                WM_TABLET_LAST,
                WM_TCARD,
                WM_THEMECHANGED,
                WM_TIMECHANGE,
                WM_TIMER,
                WM_TOOLTIPDISMISS,
                WM_TOUCH,
                WM_TOUCHHITTESTING,
                WM_UNDO,
                WM_UNICHAR,
                WM_UNINITMENUPOPUP,
                WM_UPDATEUISTATE,
                WM_USER,
                WM_USERCHANGED,
                WM_VKEYTOITEM,
                WM_VSCROLL,
                WM_VSCROLLCLIPBOARD,
                WM_WINDOWPOSCHANGED,
                WM_WINDOWPOSCHANGING,
                WM_WININICHANGE,
                WM_WTSSESSION_CHANGE,
                WM_XBUTTONDBLCLK,
                WM_XBUTTONDOWN,
                WM_XBUTTONUP,
            );
            map
        },
    )
}

static STRING_MESSAGE_NAMES: Mutex<Option<HashMap<u32, String>>> = Mutex::new(None);

#[cold]
#[inline(never)]
fn get_string_message_name(msg: &u32) -> String {
    let mut name = [0; 256];
    // SAFETY: no safety requirements
    let len = unsafe { GetClipboardFormatNameW(*msg, &mut name) };
    String::from_utf16_lossy(&name[..len as usize])
}

pub struct Name(pub u32);

impl Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            // User messages https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-user
            WM_USER..=0x7fff => write!(f, "WM_USER+{}", self.0 - WM_USER),
            // String-based messages https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-user
            0xC000..=0xFFFF => {
                let mut map = STRING_MESSAGE_NAMES
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                let name = map
                    .get_or_insert_default()
                    .entry(self.0)
                    .or_insert_with_key(get_string_message_name);
                f.write_str(name)
            }
            _ => match messages().get(&self.0) {
                // Predefined messages
                Some(&name) => f.write_str(name),
                // Unknown messages
                None => write!(f, "0x{:04x}", self.0),
            },
        }
    }
}
