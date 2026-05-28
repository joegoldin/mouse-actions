//! Generic, configurable input rules:
//!
//! - `ModifierRemap`: while a key/button is held, intercept a trigger
//!   key/button and emit something else (e.g. while LeftMouse held,
//!   RightMouse press → inject ShiftLeft for KWin window-snap).
//!
//! - `ChordBinding`: when a set of mouse buttons is pressed within a time
//!   window, spawn a shell command (e.g. forward+back within 100 ms →
//!   `dbus-send … Overview`).
//!
//! These complement the existing shape/edge/click `bindings` in `Config`.

use std::time::Instant;

use rdev::Key;
use serde::{Deserialize, Serialize};

use crate::event::MouseButton;

/// A key or mouse button referenced by configuration.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(tag = "kind", content = "code")]
pub enum InputId {
    Mouse(MouseButton),
    Key(Key),
}

/// How the emitted output behaves relative to the trigger press.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemapMode {
    /// Emit pressed while trigger is pressed; released when trigger releases
    /// (or when `while_held` releases, whichever first).
    Hold,
    /// Each trigger press toggles emit on/off. `while_held` release forces
    /// emit off. Mirrors the legacy drag-shift behavior.
    Toggle,
}

fn default_mode() -> RemapMode {
    RemapMode::Toggle
}

fn default_release_delay_ms() -> u64 {
    25
}

/// "While `while_held` is pressed, when `trigger` is pressed, emit `emit`."
///
/// Original trigger press/release events are swallowed for as long as the
/// remap is active (so the host app doesn't see e.g. a stray right-click).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModifierRemap {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    pub while_held: InputId,
    pub trigger: InputId,
    pub emit: InputId,
    #[serde(default = "default_mode")]
    pub mode: RemapMode,
    /// Delay (ms) before releasing `emit` when `while_held` releases. Gives
    /// downstream consumers (KWin) time to finalize gestures that started
    /// while the emit was held.
    #[serde(default = "default_release_delay_ms")]
    pub release_delay_ms: u64,
}

/// Per-rule runtime state. Kept parallel to `Config.modifier_remaps`.
#[derive(Debug, Default)]
pub struct ModifierRemapState {
    /// True after we emitted KeyPress/ButtonPress for this rule and haven't
    /// emitted the release yet.
    pub emit_active: bool,
    /// True if we swallowed the matching trigger press and must also swallow
    /// the trigger release.
    pub trigger_swallowed: bool,
}

fn default_window_ms() -> u64 {
    100
}

fn default_passthrough() -> bool {
    true
}

/// "When all of `buttons` are pressed within `window_ms` of each other, run
/// `cmd_str`."
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChordBinding {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    pub buttons: Vec<MouseButton>,
    #[serde(default = "default_window_ms")]
    pub window_ms: u64,
    pub cmd_str: String,
    /// If true (default), the original button events still pass through to
    /// the host — chord fires as a side effect. Set false to swallow them.
    #[serde(default = "default_passthrough")]
    pub passthrough: bool,
}

/// Returns `Some(instant)` if every button in `chord` except `just_pressed`
/// has a recorded press-time within `window_ms` of `now`; otherwise None.
pub fn chord_complete(
    chord: &ChordBinding,
    just_pressed: MouseButton,
    now: Instant,
    history: &[(MouseButton, Instant)],
) -> bool {
    if !chord.buttons.contains(&just_pressed) {
        return false;
    }
    let window = std::time::Duration::from_millis(chord.window_ms);
    chord.buttons.iter().all(|b| {
        *b == just_pressed
            || history
                .iter()
                .rev()
                .find_map(|(bb, t)| (*bb == *b).then_some(*t))
                .is_some_and(|t| now.duration_since(t) <= window)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn b(name: &str) -> MouseButton {
        match name {
            "Side" => MouseButton::Side,
            "Extra" => MouseButton::Extra,
            "Left" => MouseButton::Left,
            _ => unreachable!(),
        }
    }

    #[test]
    fn chord_complete_pair_within_window() {
        let chord = ChordBinding {
            comment: String::new(),
            buttons: vec![b("Side"), b("Extra")],
            window_ms: 100,
            cmd_str: "x".into(),
            passthrough: true,
        };
        let now = Instant::now();
        let history = vec![(b("Side"), now - Duration::from_millis(50))];
        assert!(chord_complete(&chord, b("Extra"), now, &history));
    }

    #[test]
    fn chord_incomplete_outside_window() {
        let chord = ChordBinding {
            comment: String::new(),
            buttons: vec![b("Side"), b("Extra")],
            window_ms: 100,
            cmd_str: "x".into(),
            passthrough: true,
        };
        let now = Instant::now();
        let history = vec![(b("Side"), now - Duration::from_millis(500))];
        assert!(!chord_complete(&chord, b("Extra"), now, &history));
    }

    #[test]
    fn chord_ignores_unrelated_button() {
        let chord = ChordBinding {
            comment: String::new(),
            buttons: vec![b("Side"), b("Extra")],
            window_ms: 100,
            cmd_str: "x".into(),
            passthrough: true,
        };
        let now = Instant::now();
        let history = vec![(b("Side"), now - Duration::from_millis(20))];
        assert!(!chord_complete(&chord, b("Left"), now, &history));
    }

    #[test]
    fn serde_roundtrip_input_id() {
        let id = InputId::Mouse(MouseButton::Left);
        let s = serde_json::to_string(&id).unwrap();
        assert_eq!(s, r#"{"kind":"Mouse","code":"Left"}"#);
        let back: InputId = serde_json::from_str(&s).unwrap();
        assert_eq!(back, id);

        let id2 = InputId::Key(Key::ShiftLeft);
        let s2 = serde_json::to_string(&id2).unwrap();
        let back2: InputId = serde_json::from_str(&s2).unwrap();
        assert_eq!(back2, id2);
    }
}
