use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use log::Level::Trace;
use log::{debug, error, log_enabled, trace};
use rdev::{grab, Button, Event, EventType, GrabError, Key};

use crate::uinput_simulate::simulate;

use crate::args::Args;
use crate::config::Config;
use crate::event::{
    ButtonState, ClickEvent, Edge, KeyboardModifier, KeyboardState, MouseButton, Point,
    PointHistory, PointHistoryArcMutex,
};
use crate::input_rules::{chord_complete, InputId, ModifierRemapState, RemapMode};
use crate::{event, listen, points_to_angles, trace_svg};

/// Bounded history of recent mouse-button presses, used by chord detection.
const CHORD_HISTORY_CAP: usize = 32;

pub struct GrabContext {
    pub point_history: PointHistoryArcMutex,
    pub button_state: Arc<Mutex<ButtonState>>,
    pub keyboard_state: Arc<Mutex<KeyboardState>>,
    pub config: Arc<Mutex<Config>>,
    pub last_point: Arc<Mutex<Point>>,
    pub args: Arc<Args>,
    // Generic, config-driven state:
    // rdev::Button and rdev::Key don't impl Hash, so we track held inputs
    // in small Vecs. At most a handful of buttons/keys are held at once.
    pub held_buttons: Arc<Mutex<Vec<Button>>>,
    pub held_keys: Arc<Mutex<Vec<Key>>>,
    pub remap_states: Arc<Mutex<Vec<ModifierRemapState>>>,
    pub chord_history: Arc<Mutex<Vec<(MouseButton, time::Instant)>>>,
    // Chord-swallow buffering: when a button participates in a chord_binding
    // with passthrough=false, its press is held back here until either the
    // chord completes (cancelled) or the chord's window expires (released).
    pub pending_chord_press: Arc<Mutex<Vec<(Button, time::Instant)>>>,
    // Buttons whose press was swallowed because their chord fired. Their
    // subsequent physical release should also be swallowed so the host never
    // sees a stray release without a preceding press.
    pub chord_consumed: Arc<Mutex<Vec<Button>>>,
}

pub fn start_grab_binding(
    args: Arc<Args>,
    config: Arc<Mutex<Config>>,
    process_event_fn: fn(Arc<Mutex<Config>>, ClickEvent, Arc<Args>) -> bool,
) -> Result<(), GrabError> {
    // FIXME : to avoid "Release Enter key event" to be lost (if run the script by Enter press in a terminal)
    thread::sleep(time::Duration::from_millis(300));
    let point_history: PointHistoryArcMutex = Arc::new(Mutex::new(PointHistory::new()));
    let button_state: Arc<Mutex<ButtonState>> = Arc::new(Mutex::new(ButtonState::None));
    let keyboard_state: Arc<Mutex<KeyboardState>> = Arc::new(Mutex::new(KeyboardState::default()));
    let last_point: Arc<Mutex<Point>> = Arc::new(Mutex::new(Point { x: 10, y: 10 }));
    let held_buttons: Arc<Mutex<Vec<Button>>> = Arc::new(Mutex::new(Vec::new()));
    let held_keys: Arc<Mutex<Vec<Key>>> = Arc::new(Mutex::new(Vec::new()));
    let remap_states: Arc<Mutex<Vec<ModifierRemapState>>> = Arc::new(Mutex::new(Vec::new()));
    let chord_history: Arc<Mutex<Vec<(MouseButton, time::Instant)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let pending_chord_press: Arc<Mutex<Vec<(Button, time::Instant)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let chord_consumed: Arc<Mutex<Vec<Button>>> = Arc::new(Mutex::new(Vec::new()));
    if !args.no_listen {
        listen::start_listen(last_point.clone());
    }

    debug!("Start grab");
    grab(move |event: Event| {
        let context = GrabContext {
            point_history: point_history.clone(),
            button_state: button_state.clone(),
            keyboard_state: keyboard_state.clone(),
            config: config.clone(),
            last_point: last_point.clone(),
            args: args.clone(),
            held_buttons: held_buttons.clone(),
            held_keys: held_keys.clone(),
            remap_states: remap_states.clone(),
            chord_history: chord_history.clone(),
            pending_chord_press: pending_chord_press.clone(),
            chord_consumed: chord_consumed.clone(),
        };
        grab_event_fn(event, context, process_event_fn)
    })
}

/// Emit a simulated input event for the given target (key or mouse button).
fn simulate_input(target: InputId, press: bool) {
    let evt = match (target, press) {
        (InputId::Key(k), true) => EventType::KeyPress(k),
        (InputId::Key(k), false) => EventType::KeyRelease(k),
        (InputId::Mouse(b), true) => EventType::ButtonPress(b.to_rdev_event()),
        (InputId::Mouse(b), false) => EventType::ButtonRelease(b.to_rdev_event()),
    };
    if let Err(e) = simulate(&evt) {
        error!("simulate {:?} failed: {:?}", evt, e);
    }
}

fn input_held(id: InputId, held_buttons: &[Button], held_keys: &[Key]) -> bool {
    match id {
        InputId::Mouse(b) => {
            let rdev_btn = b.to_rdev_event();
            held_buttons.iter().any(|x| *x == rdev_btn)
        }
        InputId::Key(k) => held_keys.iter().any(|x| *x == k),
    }
}

fn input_matches_button(id: InputId, btn: Button) -> bool {
    matches!(id, InputId::Mouse(b) if b.to_rdev_event() == btn)
}

fn input_matches_key(id: InputId, key: Key) -> bool {
    matches!(id, InputId::Key(k) if k == key)
}

fn spawn_shell_cmd(cmd_str: &str) {
    let _ = Command::new("sh")
        .arg("-c")
        .arg(cmd_str)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Append a press to the bounded chord history.
fn record_press(history: &mut Vec<(MouseButton, time::Instant)>, btn: MouseButton, now: time::Instant) {
    history.push((btn, now));
    if history.len() > CHORD_HISTORY_CAP {
        let drop = history.len() - CHORD_HISTORY_CAP / 2;
        history.drain(0..drop);
    }
}

/// Drop entries older than `max_age_ms` from a (Button, Instant) ring buffer.
/// Caps total length too — defends against runaway growth if we ever stop
/// trimming on the read path.
fn prune_old(buf: &mut Vec<(Button, time::Instant)>, now: time::Instant, max_age_ms: u64) {
    let cutoff = time::Duration::from_millis(max_age_ms);
    buf.retain(|(_, t)| now.duration_since(*t) <= cutoff);
    if buf.len() > 64 {
        let drop = buf.len() - 32;
        buf.drain(0..drop);
    }
}

/// `Some(window_ms)` if the button participates in any chord_binding whose
/// passthrough is false. We use the largest such window so we wait long
/// enough for the chord to potentially complete.
fn chord_swallow_window_for(
    btn: MouseButton,
    chords: &[crate::input_rules::ChordBinding],
) -> Option<u64> {
    chords
        .iter()
        .filter(|c| !c.passthrough && c.buttons.contains(&btn))
        .map(|c| c.window_ms)
        .max()
}

/// Inject a synthetic press or release for `btn` via the uinput simulator.
///
/// NOTE: injected events do NOT re-enter the grab loop — rdev's device set is
/// fixed at startup (inotify watching is disabled) and our uinput injector is
/// created lazily afterwards, so it is never grabbed/read. That means we must
/// NOT try to recognize "our own echo": there is none. An earlier attempt to
/// do so (a `recently_injected` buffer) instead matched the *real* physical
/// release of the same button and skipped re-injecting its release, leaving
/// e.g. BTN_SIDE stuck down on the injector device.
fn inject_button(btn: Button, press: bool) {
    let evt = if press {
        EventType::ButtonPress(btn)
    } else {
        EventType::ButtonRelease(btn)
    };
    if let Err(e) = simulate(&evt) {
        error!("simulate {:?} failed: {:?}", evt, e);
    }
}

pub fn grab_event_fn(
    event: Event,
    GrabContext {
        point_history,
        button_state,
        keyboard_state,
        config,
        last_point,
        args,
        held_buttons,
        held_keys,
        remap_states,
        chord_history,
        pending_chord_press,
        chord_consumed,
    }: GrabContext,
    process_event_fn: fn(Arc<Mutex<Config>>, ClickEvent, Arc<Args>) -> bool,
) -> Option<Event> {
    match event.event_type {
        EventType::MouseMove { x, y } => {
            if args.no_listen {
                last_point.lock().unwrap().set(x as i32, y as i32);
            }
            if let ButtonState::Pressed(pressed_btn) = *button_state.lock().unwrap() {
                if config.lock().unwrap().shape_button.to_rdev_event() == pressed_btn {
                    let mut histo = point_history.lock().unwrap();
                    if !histo.is_full() {
                        histo.push(*last_point.lock().unwrap());
                    } else {
                        trace!("point_history is full !")
                    }
                }
            }
            Some(event)
        }
        EventType::ButtonPress(pressed_btn) => {
            let now = time::Instant::now();

            {
                let mut hb = held_buttons.lock().unwrap();
                if !hb.iter().any(|b| *b == pressed_btn) {
                    hb.push(pressed_btn);
                }
            }
            let mouse_btn = MouseButton::from_rdev_event(pressed_btn);

            // --- Chord bindings ----------------------------------------------
            let cfg_chords = config.lock().unwrap().chord_bindings.clone();
            let mut swallow_chord = false;
            let mut fired_swallow_chord: Option<usize> = None;
            {
                let history = chord_history.lock().unwrap();
                for (idx, chord) in cfg_chords.iter().enumerate() {
                    if chord_complete(chord, mouse_btn, now, &history) {
                        spawn_shell_cmd(&chord.cmd_str);
                        if !chord.passthrough {
                            swallow_chord = true;
                            fired_swallow_chord = Some(idx);
                        }
                    }
                }
            }
            record_press(&mut chord_history.lock().unwrap(), mouse_btn, now);

            // If a passthrough=false chord just fired, cancel any pending
            // first-press buffers for the other buttons in that chord and
            // mark all participants so we also swallow their physical
            // releases below.
            if let Some(idx) = fired_swallow_chord {
                let chord = &cfg_chords[idx];
                let other_rdev: Vec<Button> = chord
                    .buttons
                    .iter()
                    .map(|b| b.to_rdev_event())
                    .collect();
                {
                    let mut pending = pending_chord_press.lock().unwrap();
                    pending.retain(|(b, _)| !other_rdev.contains(b));
                }
                {
                    let mut consumed = chord_consumed.lock().unwrap();
                    for b in &other_rdev {
                        if !consumed.contains(b) {
                            consumed.push(*b);
                        }
                    }
                }
            }

            // If no chord fired yet but this button is the FIRST member of a
            // passthrough=false chord, defer its press: a sibling button
            // arriving within window_ms will cancel us; otherwise a timer
            // re-injects it via uinput so a solo click still reaches the
            // host (with a small added latency = window_ms).
            if !swallow_chord {
                if let Some(window_ms) = chord_swallow_window_for(mouse_btn, &cfg_chords) {
                    {
                        let mut pending = pending_chord_press.lock().unwrap();
                        prune_old(&mut pending, now, window_ms.saturating_mul(4));
                        pending.retain(|(b, _)| *b != pressed_btn);
                        pending.push((pressed_btn, now));
                    }
                    let pending_arc = pending_chord_press.clone();
                    thread::spawn(move || {
                        thread::sleep(time::Duration::from_millis(window_ms));
                        let still_pending = {
                            let pending = pending_arc.lock().unwrap();
                            pending
                                .iter()
                                .any(|(b, t)| *b == pressed_btn && *t == now)
                        };
                        if still_pending {
                            inject_button(pressed_btn, true);
                            // Leave the entry in `pending_chord_press` so the
                            // physical release of this button still flows
                            // through the "we deferred this" branch in the
                            // release handler and emits the matching release.
                        }
                    });
                    return None;
                }
            }

            // --- Modifier remaps --------------------------------------------
            let cfg_remaps = config.lock().unwrap().modifier_remaps.clone();
            let mut swallow_remap = false;
            for (idx, rule) in cfg_remaps.iter().enumerate() {
                let while_held_active = {
                    let hb = held_buttons.lock().unwrap();
                    let hk = held_keys.lock().unwrap();
                    input_held(rule.while_held, &hb, &hk)
                };
                if !while_held_active {
                    continue;
                }
                if !input_matches_button(rule.trigger, pressed_btn) {
                    continue;
                }
                let target_active = {
                    let mut states = remap_states.lock().unwrap();
                    while states.len() <= idx {
                        states.push(ModifierRemapState::default());
                    }
                    let st = &mut states[idx];
                    st.trigger_swallowed = true;
                    let next = match rule.mode {
                        RemapMode::Toggle => !st.emit_active,
                        RemapMode::Hold => true,
                    };
                    st.emit_active = next;
                    next
                };
                simulate_input(rule.emit, target_active);
                swallow_remap = true;
            }

            if swallow_chord || swallow_remap {
                return None;
            }

            // --- Original shape pipeline ------------------------------------
            *button_state.lock().unwrap() = ButtonState::Pressed(pressed_btn);
            let last_point_clone = *last_point.lock().unwrap();

            let click_event = ClickEvent {
                button: mouse_btn,
                edges: Edge::edges_from_pos(last_point_clone.x, last_point_clone.y),
                modifiers: KeyboardModifier::from_keyboard_state(*keyboard_state.lock().unwrap()),
                event_type: event::EventType::Press,
                shapes_angles: vec![],
                shapes_xy: vec![],
            };
            if config.lock().unwrap().shape_button.to_rdev_event() == pressed_btn {
                let mut histo = point_history.lock().unwrap();
                if !histo.is_full() {
                    histo.push(last_point_clone);
                } else {
                    trace!("point_history is full !");
                }
                if histo.len() < 10 {
                    process_event_fn(config, click_event, args);
                }
                return None;
            }
            if process_event_fn(config, click_event, args) {
                Some(event)
            } else {
                None
            }
        }
        EventType::ButtonRelease(btn) => {
            // --- Was this button's press swallowed in service of a chord? ---
            // If so, swallow the release too.
            {
                let mut consumed = chord_consumed.lock().unwrap();
                if let Some(pos) = consumed.iter().position(|b| *b == btn) {
                    consumed.remove(pos);
                    held_buttons.lock().unwrap().retain(|b| *b != btn);
                    return None;
                }
            }

            // --- Was this press deferred and we're now releasing it before
            // a chord ever completed? Synthesize the original click pair so
            // the host doesn't lose the button event entirely.
            let was_pending = {
                let mut pending = pending_chord_press.lock().unwrap();
                if let Some(pos) = pending.iter().position(|(b, _)| *b == btn) {
                    pending.remove(pos);
                    true
                } else {
                    false
                }
            };
            if was_pending {
                inject_button(btn, true);
                // Tiny gap so libinput/KWin see them as discrete events.
                thread::sleep(time::Duration::from_millis(1));
                inject_button(btn, false);
                held_buttons.lock().unwrap().retain(|b| *b != btn);
                return None;
            }

            held_buttons.lock().unwrap().retain(|b| *b != btn);
            let mouse_btn = MouseButton::from_rdev_event(btn);

            // --- Modifier remap releases ------------------------------------
            let cfg_remaps = config.lock().unwrap().modifier_remaps.clone();
            let mut swallow_release = false;
            for (idx, rule) in cfg_remaps.iter().enumerate() {
                // ensure state slot
                {
                    let mut states = remap_states.lock().unwrap();
                    while states.len() <= idx {
                        states.push(ModifierRemapState::default());
                    }
                }

                let trigger_is_this = input_matches_button(rule.trigger, btn);
                let while_is_this = input_matches_button(rule.while_held, btn);

                if trigger_is_this {
                    let mut states = remap_states.lock().unwrap();
                    let st = &mut states[idx];
                    if st.trigger_swallowed {
                        let release_now = rule.mode == RemapMode::Hold && st.emit_active;
                        if release_now {
                            st.emit_active = false;
                        }
                        st.trigger_swallowed = false;
                        let emit = rule.emit;
                        drop(states);
                        if release_now {
                            simulate_input(emit, false);
                        }
                        swallow_release = true;
                    }
                }

                if while_is_this {
                    let st_snapshot = {
                        let states = remap_states.lock().unwrap();
                        states.get(idx).map(|s| s.emit_active).unwrap_or(false)
                    };
                    if st_snapshot {
                        let emit = rule.emit;
                        let delay = rule.release_delay_ms;
                        let states_arc = remap_states.clone();
                        let idx_copy = idx;
                        thread::spawn(move || {
                            thread::sleep(time::Duration::from_millis(delay));
                            simulate_input(emit, false);
                            if let Ok(mut sts) = states_arc.lock() {
                                if let Some(s) = sts.get_mut(idx_copy) {
                                    s.emit_active = false;
                                    s.trigger_swallowed = false;
                                }
                            }
                        });
                    }
                }
            }

            if swallow_release {
                return None;
            }

            // --- Original shape-release pipeline ----------------------------
            let angles = points_to_angles::points_to_angles(&point_history.lock().unwrap());

            if log_enabled!(Trace) {
                let normalized_points = normalize_points(&point_history.lock().unwrap(), false);
                trace!("normalized_points = {normalized_points:?}");
                trace_svg::trace_svg(&point_history.lock().unwrap(), &angles);
            }
            let last_point_clone = *last_point.lock().unwrap();
            let click_event = ClickEvent {
                button: mouse_btn,
                edges: Edge::edges_from_pos(last_point_clone.x, last_point_clone.y),
                modifiers: KeyboardModifier::from_keyboard_state(*keyboard_state.lock().unwrap()),
                event_type: event::EventType::Release,
                shapes_angles: vec![angles],
                shapes_xy: vec![point_history.lock().unwrap().clone()],
            };
            point_history.lock().unwrap().clear();
            *button_state.lock().unwrap() = ButtonState::None;

            if process_event_fn(config, click_event, args) {
                Some(event)
            } else {
                None
            }
        }
        EventType::Wheel { delta_y, .. } => {
            let last_point_clone = *last_point.lock().unwrap();
            let click_event = ClickEvent {
                button: MouseButton::from_rdev_wheel(delta_y),
                edges: Edge::edges_from_pos(last_point_clone.x, last_point_clone.y),
                modifiers: KeyboardModifier::from_keyboard_state(*keyboard_state.lock().unwrap()),
                event_type: event::EventType::Release,
                shapes_angles: vec![],
                shapes_xy: vec![],
            };
            if process_event_fn(config, click_event, args) {
                Some(event)
            } else {
                None
            }
        }
        EventType::KeyPress(key) => {
            {
                let mut hk = held_keys.lock().unwrap();
                if !hk.iter().any(|k| *k == key) {
                    hk.push(key);
                }
            }
            match key {
                Key::ShiftLeft => keyboard_state.lock().unwrap().shift_left = true,
                Key::ShiftRight => keyboard_state.lock().unwrap().shift_right = true,
                Key::ControlLeft => keyboard_state.lock().unwrap().control_left = true,
                Key::ControlRight => keyboard_state.lock().unwrap().control_right = true,
                Key::MetaLeft => keyboard_state.lock().unwrap().meta_left = true,
                Key::Alt => keyboard_state.lock().unwrap().alt = true,
                Key::AltGr => keyboard_state.lock().unwrap().alt_gr = true,
                _ => {}
            }

            // --- Modifier remaps triggered by a key press -------------------
            let cfg_remaps = config.lock().unwrap().modifier_remaps.clone();
            let mut swallow = false;
            for (idx, rule) in cfg_remaps.iter().enumerate() {
                if !input_matches_key(rule.trigger, key) {
                    continue;
                }
                let while_held_active = {
                    let hb = held_buttons.lock().unwrap();
                    let hk = held_keys.lock().unwrap();
                    input_held(rule.while_held, &hb, &hk)
                };
                if !while_held_active {
                    continue;
                }
                let target_active = {
                    let mut states = remap_states.lock().unwrap();
                    while states.len() <= idx {
                        states.push(ModifierRemapState::default());
                    }
                    let st = &mut states[idx];
                    st.trigger_swallowed = true;
                    let next = match rule.mode {
                        RemapMode::Toggle => !st.emit_active,
                        RemapMode::Hold => true,
                    };
                    st.emit_active = next;
                    next
                };
                simulate_input(rule.emit, target_active);
                swallow = true;
            }
            if swallow {
                return None;
            }
            Some(event)
        }
        EventType::KeyRelease(key) => {
            held_keys.lock().unwrap().retain(|k| *k != key);
            match key {
                Key::ShiftLeft => keyboard_state.lock().unwrap().shift_left = false,
                Key::ShiftRight => keyboard_state.lock().unwrap().shift_right = false,
                Key::ControlLeft => keyboard_state.lock().unwrap().control_left = false,
                Key::ControlRight => keyboard_state.lock().unwrap().control_right = false,
                Key::MetaLeft => keyboard_state.lock().unwrap().meta_left = false,
                Key::Alt => keyboard_state.lock().unwrap().alt = false,
                Key::AltGr => keyboard_state.lock().unwrap().alt_gr = false,
                _ => {}
            }

            // --- Modifier remap release path on key release -----------------
            let cfg_remaps = config.lock().unwrap().modifier_remaps.clone();
            let mut swallow_release = false;
            for (idx, rule) in cfg_remaps.iter().enumerate() {
                {
                    let mut states = remap_states.lock().unwrap();
                    while states.len() <= idx {
                        states.push(ModifierRemapState::default());
                    }
                }

                let trigger_is_this = input_matches_key(rule.trigger, key);
                let while_is_this = input_matches_key(rule.while_held, key);

                if trigger_is_this {
                    let mut states = remap_states.lock().unwrap();
                    let st = &mut states[idx];
                    if st.trigger_swallowed {
                        let release_now = rule.mode == RemapMode::Hold && st.emit_active;
                        if release_now {
                            st.emit_active = false;
                        }
                        st.trigger_swallowed = false;
                        let emit = rule.emit;
                        drop(states);
                        if release_now {
                            simulate_input(emit, false);
                        }
                        swallow_release = true;
                    }
                }

                if while_is_this {
                    let st_snapshot = {
                        let states = remap_states.lock().unwrap();
                        states.get(idx).map(|s| s.emit_active).unwrap_or(false)
                    };
                    if st_snapshot {
                        let emit = rule.emit;
                        let delay = rule.release_delay_ms;
                        let states_arc = remap_states.clone();
                        let idx_copy = idx;
                        thread::spawn(move || {
                            thread::sleep(time::Duration::from_millis(delay));
                            simulate_input(emit, false);
                            if let Ok(mut sts) = states_arc.lock() {
                                if let Some(s) = sts.get_mut(idx_copy) {
                                    s.emit_active = false;
                                    s.trigger_swallowed = false;
                                }
                            }
                        });
                    }
                }
            }

            if swallow_release {
                return None;
            }
            Some(event)
        }
    }
}

pub fn normalize_points(input_points: &PointHistory, use_avg: bool) -> PointHistory {
    let mut out = PointHistory::new();
    if !input_points.is_empty() {
        let min_x = input_points.iter().map(|p| p.x).min().unwrap();
        let max_x = input_points.iter().map(|p| p.x).max().unwrap();
        let width = max_x - min_x;

        let min_y = input_points.iter().map(|p| p.y).min().unwrap();
        let max_y = input_points.iter().map(|p| p.y).max().unwrap();
        let height = max_y - min_y;
        let size = width.max(height);

        if width > 0 && height > 0 {
            if use_avg {
                let avg_x: i32 =
                    input_points.iter().map(|p| p.x).sum::<i32>() / (input_points.len() as i32);
                let avg_y: i32 =
                    input_points.iter().map(|p| p.y).sum::<i32>() / (input_points.len() as i32);
                for p in input_points.iter() {
                    out.push(Point {
                        x: 1000 * (p.x - avg_x) / size,
                        y: 1000 * (p.y - avg_y) / size,
                    });
                }
            } else {
                for p in input_points.iter() {
                    out.push(Point {
                        x: 1000 * (p.x - min_x) / size,
                        y: 1000 * (p.y - min_y) / size,
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::event::{Point, PointHistory};
    use crate::grab::normalize_points;

    #[test]
    fn test_normalize_points() {
        let mut points = PointHistory::new();
        points.push(Point { x: 0, y: 0 });
        points.push(Point { x: 10, y: 0 });
        points.push(Point { x: 5, y: 4 });
        points.push(Point { x: 5, y: 2 });
        let norm = normalize_points(&points, false);
        assert_eq!(norm.get(0).unwrap(), &Point { x: 0, y: 0 });
        assert_eq!(norm.get(1).unwrap(), &Point { x: 1000, y: 0 });
        assert_eq!(norm.get(2).unwrap(), &Point { x: 500, y: 400 });
        assert_eq!(norm.get(3).unwrap(), &Point { x: 500, y: 200 });
    }
}
