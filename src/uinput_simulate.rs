//! Linux uinput-based input simulator.
//!
//! `rdev::simulate` on Linux uses X11 XTest, which on a Wayland session goes
//! through Xwayland and gets routed through the `org.freedesktop.portal.RemoteDesktop`
//! portal. On KDE this surfaces as a recurring "Remote Control / Control input
//! devices" dialog every time the portal session needs re-authorization, and
//! synthetic events never reach Wayland-native windows.
//!
//! This module bypasses X11 entirely: we create a single virtual uinput device
//! at first use and inject EV_KEY / EV_REL events directly. KWin (and any other
//! libinput consumer) sees the events at the kernel level just like a real
//! mouse/keyboard — no portal involvement, works on both X11 and Wayland
//! windows.
//!
//! Requires `/dev/uinput` to be writable by the running user (the standard
//! NixOS udev rule shipped by `mouse-actions` itself takes care of that).

use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use evdev_rs::enums::{EventCode, EventType as EvType, EV_KEY, EV_REL, EV_SYN};
use evdev_rs::{Device, InputEvent, TimeVal, UInputDevice};
use log::{error, warn};
use rdev::{Button, EventType, Key, SimulateError};

/// `UInputDevice` holds a raw `*mut libevdev_uinput`. The pointer itself is
/// not `Send`, but we serialize every access through a `Mutex` so single-
/// threaded access is enforced. Wrap it in a Send-safe newtype.
struct SendableUInput(UInputDevice);
unsafe impl Send for SendableUInput {}

static UINPUT: OnceLock<Mutex<Option<SendableUInput>>> = OnceLock::new();

/// Public entry point: drop-in replacement for `rdev::simulate`.
pub fn simulate(event: &EventType) -> Result<(), SimulateError> {
    let cell = UINPUT.get_or_init(|| Mutex::new(create_device().map(SendableUInput)));
    let guard = cell.lock().map_err(|_| SimulateError)?;
    let dev = guard.as_ref().ok_or(SimulateError)?;
    inject(&dev.0, event)
}

fn now_timeval() -> TimeVal {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => TimeVal::new(d.as_secs() as i64, d.subsec_micros() as i64),
        Err(_) => TimeVal::new(0, 0),
    }
}

fn inject(dev: &UInputDevice, event: &EventType) -> Result<(), SimulateError> {
    let t = now_timeval();
    let (code, value) = match event {
        EventType::KeyPress(k) => (EventCode::EV_KEY(key_to_evdev(*k).ok_or(SimulateError)?), 1),
        EventType::KeyRelease(k) => (EventCode::EV_KEY(key_to_evdev(*k).ok_or(SimulateError)?), 0),
        EventType::ButtonPress(b) => (EventCode::EV_KEY(button_to_evdev(*b).ok_or(SimulateError)?), 1),
        EventType::ButtonRelease(b) => (EventCode::EV_KEY(button_to_evdev(*b).ok_or(SimulateError)?), 0),
        EventType::Wheel { delta_x: _, delta_y } => {
            let v = (*delta_y).max(i32::MIN as i64).min(i32::MAX as i64) as i32;
            (EventCode::EV_REL(EV_REL::REL_WHEEL), v)
        }
        EventType::MouseMove { x, y } => {
            // Relative motion only — uinput doesn't model an absolute pointer
            // without an EV_ABS axis. Callers that need absolute positioning
            // still have to go through X / portal.
            let dx = (*x).max(i32::MIN as f64).min(i32::MAX as f64) as i32;
            let dy = (*y).max(i32::MIN as f64).min(i32::MAX as f64) as i32;
            dev.write_event(&InputEvent::new(&t, &EventCode::EV_REL(EV_REL::REL_X), dx))
                .map_err(|_| SimulateError)?;
            dev.write_event(&InputEvent::new(&t, &EventCode::EV_REL(EV_REL::REL_Y), dy))
                .map_err(|_| SimulateError)?;
            return write_syn(dev, &t);
        }
    };
    dev.write_event(&InputEvent::new(&t, &code, value))
        .map_err(|_| SimulateError)?;
    write_syn(dev, &t)
}

fn write_syn(dev: &UInputDevice, t: &TimeVal) -> Result<(), SimulateError> {
    dev.write_event(&InputEvent::new(
        t,
        &EventCode::EV_SYN(EV_SYN::SYN_REPORT),
        0,
    ))
    .map_err(|_| SimulateError)
}

fn create_device() -> Option<UInputDevice> {
    let dev = Device::new()?;
    dev.set_name("mouse-actions virtual injector");
    dev.set_bustype(0x06); // BUS_VIRTUAL
    dev.set_vendor_id(0x1234);
    dev.set_product_id(0xabcd);
    dev.set_version(1);

    if let Err(e) = dev.enable_event_type(&EvType::EV_KEY) {
        error!("uinput: enable EV_KEY failed: {:?}", e);
        return None;
    }
    if let Err(e) = dev.enable_event_type(&EvType::EV_REL) {
        error!("uinput: enable EV_REL failed: {:?}", e);
        return None;
    }
    if let Err(e) = dev.enable_event_type(&EvType::EV_SYN) {
        error!("uinput: enable EV_SYN failed: {:?}", e);
        return None;
    }

    for k in supported_ev_keys() {
        let _ = dev.enable_event_code(&EventCode::EV_KEY(k), None);
    }
    for r in [EV_REL::REL_X, EV_REL::REL_Y, EV_REL::REL_WHEEL, EV_REL::REL_HWHEEL] {
        let _ = dev.enable_event_code(&EventCode::EV_REL(r), None);
    }

    match UInputDevice::create_from_device(&dev) {
        Ok(u) => Some(u),
        Err(e) => {
            warn!(
                "uinput: failed to create virtual injector device ({:?}). \
                 simulate() calls will be no-ops. Check /dev/uinput permissions.",
                e
            );
            None
        }
    }
}

/// Concrete EV_KEY values we declare on the virtual injector. The pure-write
/// `inject` path only emits codes from this list (or fails with SimulateError),
/// so any value here must be declared at device creation time too.
fn supported_ev_keys() -> impl Iterator<Item = EV_KEY> {
    [
        // Mouse buttons
        EV_KEY::BTN_LEFT,
        EV_KEY::BTN_RIGHT,
        EV_KEY::BTN_MIDDLE,
        EV_KEY::BTN_SIDE,
        EV_KEY::BTN_EXTRA,
        EV_KEY::BTN_FORWARD,
        EV_KEY::BTN_BACK,
        EV_KEY::BTN_TASK,
        EV_KEY::BTN_TRIGGER,
        EV_KEY::BTN_THUMB,
        EV_KEY::BTN_THUMB2,
        // Modifier-ish keys
        EV_KEY::KEY_LEFTSHIFT,
        EV_KEY::KEY_RIGHTSHIFT,
        EV_KEY::KEY_LEFTCTRL,
        EV_KEY::KEY_RIGHTCTRL,
        EV_KEY::KEY_LEFTALT,
        EV_KEY::KEY_RIGHTALT,
        EV_KEY::KEY_LEFTMETA,
        EV_KEY::KEY_RIGHTMETA,
        EV_KEY::KEY_CAPSLOCK,
        EV_KEY::KEY_NUMLOCK,
        EV_KEY::KEY_SCROLLLOCK,
        // Common navigation / control
        EV_KEY::KEY_ESC,
        EV_KEY::KEY_TAB,
        EV_KEY::KEY_BACKSPACE,
        EV_KEY::KEY_ENTER,
        EV_KEY::KEY_SPACE,
        EV_KEY::KEY_INSERT,
        EV_KEY::KEY_DELETE,
        EV_KEY::KEY_HOME,
        EV_KEY::KEY_END,
        EV_KEY::KEY_PAGEUP,
        EV_KEY::KEY_PAGEDOWN,
        EV_KEY::KEY_UP,
        EV_KEY::KEY_DOWN,
        EV_KEY::KEY_LEFT,
        EV_KEY::KEY_RIGHT,
        EV_KEY::KEY_PAUSE,
        EV_KEY::KEY_PRINT,
        // Function keys
        EV_KEY::KEY_F1, EV_KEY::KEY_F2, EV_KEY::KEY_F3, EV_KEY::KEY_F4,
        EV_KEY::KEY_F5, EV_KEY::KEY_F6, EV_KEY::KEY_F7, EV_KEY::KEY_F8,
        EV_KEY::KEY_F9, EV_KEY::KEY_F10, EV_KEY::KEY_F11, EV_KEY::KEY_F12,
        // Letter row
        EV_KEY::KEY_A, EV_KEY::KEY_B, EV_KEY::KEY_C, EV_KEY::KEY_D,
        EV_KEY::KEY_E, EV_KEY::KEY_F, EV_KEY::KEY_G, EV_KEY::KEY_H,
        EV_KEY::KEY_I, EV_KEY::KEY_J, EV_KEY::KEY_K, EV_KEY::KEY_L,
        EV_KEY::KEY_M, EV_KEY::KEY_N, EV_KEY::KEY_O, EV_KEY::KEY_P,
        EV_KEY::KEY_Q, EV_KEY::KEY_R, EV_KEY::KEY_S, EV_KEY::KEY_T,
        EV_KEY::KEY_U, EV_KEY::KEY_V, EV_KEY::KEY_W, EV_KEY::KEY_X,
        EV_KEY::KEY_Y, EV_KEY::KEY_Z,
        // Digit row
        EV_KEY::KEY_0, EV_KEY::KEY_1, EV_KEY::KEY_2, EV_KEY::KEY_3,
        EV_KEY::KEY_4, EV_KEY::KEY_5, EV_KEY::KEY_6, EV_KEY::KEY_7,
        EV_KEY::KEY_8, EV_KEY::KEY_9,
        // Punctuation / common ASCII
        EV_KEY::KEY_MINUS, EV_KEY::KEY_EQUAL,
        EV_KEY::KEY_LEFTBRACE, EV_KEY::KEY_RIGHTBRACE,
        EV_KEY::KEY_SEMICOLON, EV_KEY::KEY_APOSTROPHE,
        EV_KEY::KEY_GRAVE, EV_KEY::KEY_BACKSLASH,
        EV_KEY::KEY_COMMA, EV_KEY::KEY_DOT, EV_KEY::KEY_SLASH,
        // Numpad
        EV_KEY::KEY_KP0, EV_KEY::KEY_KP1, EV_KEY::KEY_KP2, EV_KEY::KEY_KP3,
        EV_KEY::KEY_KP4, EV_KEY::KEY_KP5, EV_KEY::KEY_KP6, EV_KEY::KEY_KP7,
        EV_KEY::KEY_KP8, EV_KEY::KEY_KP9,
        EV_KEY::KEY_KPENTER, EV_KEY::KEY_KPSLASH,
        EV_KEY::KEY_KPMINUS, EV_KEY::KEY_KPPLUS, EV_KEY::KEY_KPASTERISK,
    ]
    .into_iter()
}

fn key_to_evdev(k: Key) -> Option<EV_KEY> {
    use Key::*;
    Some(match k {
        Alt => EV_KEY::KEY_LEFTALT,
        AltGr => EV_KEY::KEY_RIGHTALT,
        Backspace => EV_KEY::KEY_BACKSPACE,
        CapsLock => EV_KEY::KEY_CAPSLOCK,
        ControlLeft => EV_KEY::KEY_LEFTCTRL,
        ControlRight => EV_KEY::KEY_RIGHTCTRL,
        Delete => EV_KEY::KEY_DELETE,
        DownArrow => EV_KEY::KEY_DOWN,
        End => EV_KEY::KEY_END,
        Escape => EV_KEY::KEY_ESC,
        F1 => EV_KEY::KEY_F1, F2 => EV_KEY::KEY_F2, F3 => EV_KEY::KEY_F3,
        F4 => EV_KEY::KEY_F4, F5 => EV_KEY::KEY_F5, F6 => EV_KEY::KEY_F6,
        F7 => EV_KEY::KEY_F7, F8 => EV_KEY::KEY_F8, F9 => EV_KEY::KEY_F9,
        F10 => EV_KEY::KEY_F10, F11 => EV_KEY::KEY_F11, F12 => EV_KEY::KEY_F12,
        Home => EV_KEY::KEY_HOME,
        LeftArrow => EV_KEY::KEY_LEFT,
        MetaLeft => EV_KEY::KEY_LEFTMETA,
        MetaRight => EV_KEY::KEY_RIGHTMETA,
        PageDown => EV_KEY::KEY_PAGEDOWN,
        PageUp => EV_KEY::KEY_PAGEUP,
        Return => EV_KEY::KEY_ENTER,
        RightArrow => EV_KEY::KEY_RIGHT,
        ShiftLeft => EV_KEY::KEY_LEFTSHIFT,
        ShiftRight => EV_KEY::KEY_RIGHTSHIFT,
        Space => EV_KEY::KEY_SPACE,
        Tab => EV_KEY::KEY_TAB,
        UpArrow => EV_KEY::KEY_UP,
        PrintScreen => EV_KEY::KEY_PRINT,
        ScrollLock => EV_KEY::KEY_SCROLLLOCK,
        Pause => EV_KEY::KEY_PAUSE,
        NumLock => EV_KEY::KEY_NUMLOCK,
        BackQuote => EV_KEY::KEY_GRAVE,
        Num1 => EV_KEY::KEY_1, Num2 => EV_KEY::KEY_2, Num3 => EV_KEY::KEY_3,
        Num4 => EV_KEY::KEY_4, Num5 => EV_KEY::KEY_5, Num6 => EV_KEY::KEY_6,
        Num7 => EV_KEY::KEY_7, Num8 => EV_KEY::KEY_8, Num9 => EV_KEY::KEY_9,
        Num0 => EV_KEY::KEY_0,
        Minus => EV_KEY::KEY_MINUS, Equal => EV_KEY::KEY_EQUAL,
        KeyQ => EV_KEY::KEY_Q, KeyW => EV_KEY::KEY_W, KeyE => EV_KEY::KEY_E,
        KeyR => EV_KEY::KEY_R, KeyT => EV_KEY::KEY_T, KeyY => EV_KEY::KEY_Y,
        KeyU => EV_KEY::KEY_U, KeyI => EV_KEY::KEY_I, KeyO => EV_KEY::KEY_O,
        KeyP => EV_KEY::KEY_P, KeyA => EV_KEY::KEY_A, KeyS => EV_KEY::KEY_S,
        KeyD => EV_KEY::KEY_D, KeyF => EV_KEY::KEY_F, KeyG => EV_KEY::KEY_G,
        KeyH => EV_KEY::KEY_H, KeyJ => EV_KEY::KEY_J, KeyK => EV_KEY::KEY_K,
        KeyL => EV_KEY::KEY_L, KeyZ => EV_KEY::KEY_Z, KeyX => EV_KEY::KEY_X,
        KeyC => EV_KEY::KEY_C, KeyV => EV_KEY::KEY_V, KeyB => EV_KEY::KEY_B,
        KeyN => EV_KEY::KEY_N, KeyM => EV_KEY::KEY_M,
        LeftBracket => EV_KEY::KEY_LEFTBRACE,
        RightBracket => EV_KEY::KEY_RIGHTBRACE,
        SemiColon => EV_KEY::KEY_SEMICOLON,
        Quote => EV_KEY::KEY_APOSTROPHE,
        BackSlash => EV_KEY::KEY_BACKSLASH,
        IntlBackslash => EV_KEY::KEY_BACKSLASH,
        Comma => EV_KEY::KEY_COMMA,
        Dot => EV_KEY::KEY_DOT,
        Slash => EV_KEY::KEY_SLASH,
        Insert => EV_KEY::KEY_INSERT,
        KpReturn => EV_KEY::KEY_KPENTER,
        KpMinus => EV_KEY::KEY_KPMINUS,
        KpPlus => EV_KEY::KEY_KPPLUS,
        KpMultiply => EV_KEY::KEY_KPASTERISK,
        KpDivide => EV_KEY::KEY_KPSLASH,
        Kp0 => EV_KEY::KEY_KP0, Kp1 => EV_KEY::KEY_KP1, Kp2 => EV_KEY::KEY_KP2,
        Kp3 => EV_KEY::KEY_KP3, Kp4 => EV_KEY::KEY_KP4, Kp5 => EV_KEY::KEY_KP5,
        Kp6 => EV_KEY::KEY_KP6, Kp7 => EV_KEY::KEY_KP7, Kp8 => EV_KEY::KEY_KP8,
        Kp9 => EV_KEY::KEY_KP9,
        KpDelete => EV_KEY::KEY_DELETE,
        // rdev::Key::Unknown carries an X11 keysym, not a kernel code, so we
        // can't map it back unambiguously. Drop it.
        Unknown(_) => return None,
        // Future-proof: any new variants land here.
        #[allow(unreachable_patterns)]
        _ => return None,
    })
}

fn button_to_evdev(b: Button) -> Option<EV_KEY> {
    Some(match b {
        Button::Left => EV_KEY::BTN_LEFT,
        Button::Right => EV_KEY::BTN_RIGHT,
        Button::Middle => EV_KEY::BTN_MIDDLE,
        Button::Side => EV_KEY::BTN_SIDE,
        Button::Extra => EV_KEY::BTN_EXTRA,
        Button::Forward => EV_KEY::BTN_FORWARD,
        Button::Back => EV_KEY::BTN_BACK,
        Button::Task => EV_KEY::BTN_TASK,
        Button::Trigger => EV_KEY::BTN_TRIGGER,
        Button::Thumb => EV_KEY::BTN_THUMB,
        Button::Thumb2 => EV_KEY::BTN_THUMB2,
        Button::Unknown(_) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_shift_left() {
        assert_eq!(key_to_evdev(Key::ShiftLeft), Some(EV_KEY::KEY_LEFTSHIFT));
    }

    #[test]
    fn maps_right_button() {
        assert_eq!(button_to_evdev(Button::Right), Some(EV_KEY::BTN_RIGHT));
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(key_to_evdev(Key::Unknown(123)).is_none());
    }
}
