//! One-thumb gesture detection. macroquad emulates a mouse from single
//! touches by default (`simulate_mouse_with_touch`), and `mouse_position()`
//! is already DPI-corrected, so both desktop mouse and on-device touch flow
//! through the same code path.

use macroquad::prelude::*;

pub enum Gesture {
    /// Finger went down and came up in place, quickly.
    Tap(Vec2),
    SwipeUp,
    SwipeLeft,
    SwipeRight,
    /// Horizontal drag in progress; delta x since last frame (logical px).
    Drag(f32),
    None,
}

pub struct GestureDetector {
    down_at: Option<(Vec2, f64)>,
    last_pos: Vec2,
    dragging: bool,
}

impl GestureDetector {
    pub fn new() -> Self {
        Self {
            down_at: None,
            last_pos: Vec2::ZERO,
            dragging: false,
        }
    }

    pub fn poll(&mut self) -> Gesture {
        // Thresholds scale with screen size so gestures feel the same on a
        // phone and in the desktop dev window.
        let swipe_min = screen_width() * 0.08;
        let drag_min = screen_width() * 0.03;
        let pos: Vec2 = mouse_position().into();

        if is_mouse_button_pressed(MouseButton::Left) {
            self.down_at = Some((pos, get_time()));
            self.last_pos = pos;
            self.dragging = false;
            return Gesture::None;
        }

        if is_mouse_button_down(MouseButton::Left) {
            let Some((start, _)) = self.down_at else {
                return Gesture::None;
            };
            let total = pos - start;
            // A mostly-horizontal move becomes a track drag; vertical moves
            // stay pending so they can end as a swipe.
            if !self.dragging && total.x.abs() > drag_min && total.x.abs() > total.y.abs() {
                self.dragging = true;
            }
            let dx = pos.x - self.last_pos.x;
            self.last_pos = pos;
            return if self.dragging {
                Gesture::Drag(dx)
            } else {
                Gesture::None
            };
        }

        if is_mouse_button_released(MouseButton::Left) {
            let Some((start, t0)) = self.down_at.take() else {
                return Gesture::None;
            };
            if self.dragging {
                self.dragging = false;
                return Gesture::None;
            }
            let delta = pos - start;
            if delta.length() < swipe_min {
                if get_time() - t0 < 0.4 {
                    return Gesture::Tap(pos);
                }
                return Gesture::None;
            }
            return if delta.y.abs() > delta.x.abs() {
                if delta.y < 0.0 {
                    Gesture::SwipeUp
                } else {
                    Gesture::None
                }
            } else if delta.x < 0.0 {
                Gesture::SwipeLeft
            } else {
                Gesture::SwipeRight
            };
        }

        Gesture::None
    }
}
