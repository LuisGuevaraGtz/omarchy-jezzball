//! Keyboard/mouse mapping → game and menu actions.
//!
//! The pure layer (`UiKey`, `Frame`, `NavRepeat`) does not touch macroquad and is
//! testable without a window. Only `sample_frame` queries the real state of the
//! keyboard/mouse and runs inside the render loop.

use macroquad::input::{
    is_key_down, is_key_pressed, is_mouse_button_pressed, mouse_position, KeyCode, MouseButton,
};

/// Keys that matter for the game and the menus, decoupled from macroquad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiKey {
    Up,
    Down,
    Left,
    Right,
    H,
    J,
    K,
    L,
    Enter,
    Space,
    Escape,
    Tab,
    R,
    F,
    M,
    Q,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
}

impl UiKey {
    /// The digit key `1..=5`, or `None` if out of range.
    pub fn from_digit(n: usize) -> Option<UiKey> {
        match n {
            1 => Some(UiKey::Digit1),
            2 => Some(UiKey::Digit2),
            3 => Some(UiKey::Digit3),
            4 => Some(UiKey::Digit4),
            5 => Some(UiKey::Digit5),
            _ => None,
        }
    }
}

/// Translates a macroquad `KeyCode` into a `UiKey` (pure function).
pub fn ui_key_from(code: KeyCode) -> Option<UiKey> {
    use KeyCode::*;
    Some(match code {
        Up => UiKey::Up,
        Down => UiKey::Down,
        Left => UiKey::Left,
        Right => UiKey::Right,
        H => UiKey::H,
        J => UiKey::J,
        K => UiKey::K,
        L => UiKey::L,
        Enter | KpEnter => UiKey::Enter,
        Space => UiKey::Space,
        Escape => UiKey::Escape,
        Tab => UiKey::Tab,
        R => UiKey::R,
        F => UiKey::F,
        M => UiKey::M,
        Q => UiKey::Q,
        Key1 => UiKey::Digit1,
        Key2 => UiKey::Digit2,
        Key3 => UiKey::Digit3,
        Key4 => UiKey::Digit4,
        Key5 => UiKey::Digit5,
        _ => return None,
    })
}

/// "Up" navigation keys (arrows + `k`/`h`).
pub const NAV_UP: [UiKey; 3] = [UiKey::Up, UiKey::K, UiKey::H];
/// "Down" navigation keys (arrows + `j`/`l`).
pub const NAV_DOWN: [UiKey; 3] = [UiKey::Down, UiKey::J, UiKey::L];

/// Mouse state in the current frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct MouseFrame {
    pub x: f32,
    pub y: f32,
    pub left: bool,
    pub right: bool,
}

/// Input snapshot of a frame, built by `sample_frame`.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// Keys that went down in this frame (single-use event).
    pub pressed: Vec<UiKey>,
    /// Navigation keys currently held down (for auto-repeat).
    pub held: Vec<UiKey>,
    pub mouse: MouseFrame,
}

impl Frame {
    pub fn pressed(&self, key: UiKey) -> bool {
        self.pressed.contains(&key)
    }

    pub fn pressed_any(&self, keys: &[UiKey]) -> bool {
        keys.iter().any(|k| self.pressed.contains(k))
    }

    pub fn held(&self, key: UiKey) -> bool {
        self.held.contains(&key)
    }

    /// Index (0..=4) of the first power-up key (1..=5) pressed this frame.
    pub fn digit(&self) -> Option<usize> {
        (1..=5).find_map(|d| {
            let k = UiKey::from_digit(d)?;
            self.pressed(k).then_some(d - 1)
        })
    }
}

/// Queries macroquad's keyboard and mouse. It is only invoked inside the
/// render loop (with the window already created).
pub fn sample_frame() -> Frame {
    let pressed_codes = [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::H,
        KeyCode::J,
        KeyCode::K,
        KeyCode::L,
        KeyCode::Enter,
        KeyCode::KpEnter,
        KeyCode::Space,
        KeyCode::Escape,
        KeyCode::Tab,
        KeyCode::R,
        KeyCode::F,
        KeyCode::M,
        KeyCode::Q,
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
    ];
    let mut pressed = Vec::new();
    for code in pressed_codes {
        if is_key_pressed(code) {
            if let Some(key) = ui_key_from(code) {
                pressed.push(key);
            }
        }
    }

    let mut held = Vec::new();
    for key in [
        UiKey::Up,
        UiKey::Down,
        UiKey::Left,
        UiKey::Right,
        UiKey::H,
        UiKey::J,
        UiKey::K,
        UiKey::L,
    ] {
        let code = match key {
            UiKey::Up => KeyCode::Up,
            UiKey::Down => KeyCode::Down,
            UiKey::Left => KeyCode::Left,
            UiKey::Right => KeyCode::Right,
            UiKey::H => KeyCode::H,
            UiKey::J => KeyCode::J,
            UiKey::K => KeyCode::K,
            UiKey::L => KeyCode::L,
            _ => continue,
        };
        if is_key_down(code) {
            held.push(key);
        }
    }

    let (mx, my) = mouse_position();
    Frame {
        pressed,
        held,
        mouse: MouseFrame {
            x: mx,
            y: my,
            left: is_mouse_button_pressed(MouseButton::Left),
            right: is_mouse_button_pressed(MouseButton::Right),
        },
    }
}

/// Simple auto-repeat for navigating menus with held keys: the first
/// press fires immediately and afterwards it repeats with `cooldown`.
///
/// It returns `Some(-1)` to move up, `Some(1)` to move down and `None` if there is
/// no movement this frame.
#[derive(Clone, Copy, Debug)]
pub struct NavRepeat {
    last: i8,
    cooldown: f32,
}

impl Default for NavRepeat {
    fn default() -> Self {
        NavRepeat {
            last: 0,
            cooldown: 0.0,
        }
    }
}

impl NavRepeat {
    /// Waiting period before the first repetition when holding a key down.
    const FIRST_REPEAT: f32 = 0.3;
    /// Period between consecutive repetitions.
    const REPEAT_GAP: f32 = 0.12;

    /// Floating-point tolerance for deciding whether the counter expired.
    const EPS: f32 = 1e-4;

    pub fn input(&mut self, frame: &Frame, up: &[UiKey], down: &[UiKey], dt: f32) -> Option<i8> {
        let up_now = up.iter().any(|k| frame.pressed(*k) || frame.held(*k));
        let down_now = down.iter().any(|k| frame.pressed(*k) || frame.held(*k));
        if up_now == down_now {
            self.last = 0;
            self.cooldown = 0.0;
            return None;
        }
        let dir: i8 = if up_now { -1 } else { 1 };
        if self.last != dir {
            self.last = dir;
            self.cooldown = Self::FIRST_REPEAT;
            return Some(dir);
        }
        self.cooldown -= dt;
        if self.cooldown <= Self::EPS {
            self.cooldown = Self::REPEAT_GAP;
            Some(dir)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_with(pressed: &[UiKey], held: &[UiKey]) -> Frame {
        Frame {
            pressed: pressed.to_vec(),
            held: held.to_vec(),
            ..Frame::default()
        }
    }

    #[test]
    fn key_mapping_is_correct() {
        assert_eq!(ui_key_from(KeyCode::Space), Some(UiKey::Space));
        assert_eq!(ui_key_from(KeyCode::Escape), Some(UiKey::Escape));
        assert_eq!(ui_key_from(KeyCode::Tab), Some(UiKey::Tab));
        assert_eq!(ui_key_from(KeyCode::H), Some(UiKey::H));
        assert_eq!(ui_key_from(KeyCode::J), Some(UiKey::J));
        assert_eq!(ui_key_from(KeyCode::Enter), Some(UiKey::Enter));
        assert_eq!(ui_key_from(KeyCode::Key1), Some(UiKey::Digit1));
        assert_eq!(ui_key_from(KeyCode::Key5), Some(UiKey::Digit5));
        assert_eq!(ui_key_from(KeyCode::A), None);
    }

    #[test]
    fn digits() {
        assert_eq!(UiKey::from_digit(1), Some(UiKey::Digit1));
        assert_eq!(UiKey::from_digit(5), Some(UiKey::Digit5));
        assert_eq!(UiKey::from_digit(6), None);
    }

    #[test]
    fn nav_repeat_primer_pulso_y_auto_repeat() {
        let mut nav = NavRepeat::default();
        // First pulse (key just pressed): moves immediately.
        let f = frame_with(&[UiKey::Up], &[]);
        assert_eq!(nav.input(&f, &NAV_UP, &NAV_DOWN, 0.1), Some(-1));
        // Holding the key down: it waits for the first-repeat delay (0.3).
        let f2 = frame_with(&[], &[UiKey::Up]);
        assert_eq!(nav.input(&f2, &NAV_UP, &NAV_DOWN, 0.1), None);
        assert_eq!(nav.input(&f2, &NAV_UP, &NAV_DOWN, 0.1), None);
        // Once the accumulated 0.3 have passed, it repeats.
        assert_eq!(nav.input(&f2, &NAV_UP, &NAV_DOWN, 0.1), Some(-1));
        // And it needs another repetition gap again.
        assert_eq!(nav.input(&f2, &NAV_UP, &NAV_DOWN, 0.05), None);
    }

    #[test]
    fn nav_repeat_on_direction_change() {
        let mut nav = NavRepeat::default();
        assert_eq!(
            nav.input(&frame_with(&[], &[]), &NAV_UP, &NAV_DOWN, 0.1),
            None
        );
        assert_eq!(
            nav.input(&frame_with(&[UiKey::J], &[]), &NAV_UP, &NAV_DOWN, 0.1),
            Some(1)
        );
        // Switching to up fires instantly (new direction).
        assert_eq!(
            nav.input(&frame_with(&[UiKey::K], &[]), &NAV_UP, &NAV_DOWN, 0.1),
            Some(-1)
        );
    }

    #[test]
    fn pressed_y_pressed_any() {
        let f = frame_with(&[UiKey::Space, UiKey::R], &[]);
        assert!(f.pressed(UiKey::Space));
        assert!(!f.pressed(UiKey::Escape));
        assert!(f.pressed_any(&[UiKey::Escape, UiKey::R]));
        assert!(!f.pressed_any(&[UiKey::Tab]));
    }
}
