//! Touch and mouse gestures in logical window pixels.
use bevy::prelude::*;
use std::collections::BTreeMap;

/// Per-pointer gesture recognition. Drag threshold is 12 logical pixels.
#[derive(Resource, Default)]
pub struct PointerTracker {
    active: BTreeMap<u64, Press>,
}

/// A gesture emitted by the unified input adapter.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct PointerGesture {
    /// Touch ID, or u64::MAX for the mouse.
    pub id: u64,
    /// Gesture category.
    pub kind: PointerKind,
    /// Logical window position, with origin at the top left.
    pub position: Vec2,
    /// Displacement from the press origin.
    pub delta: Vec2,
}

/// A completed tap, an ongoing drag, or a stationary hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Tap,
    Drag,
    LongPress,
}

/// Order game input after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum FactorySet {
    Input,
}

struct Press {
    origin: Vec2,
    position: Vec2,
    start: f64,
    dragged: bool,
    held: bool,
}

impl PointerTracker {
    /// Begin or replace a press at a monotonic timestamp in seconds.
    pub fn begin(&mut self, id: u64, position: Vec2, seconds: f64) {
        self.active.insert(
            id,
            Press {
                origin: position,
                position,
                start: seconds,
                dragged: false,
                held: false,
            },
        );
    }
    /// Update a press. Emit drags after crossing the distance threshold.
    pub fn movement(&mut self, id: u64, position: Vec2) -> Option<PointerGesture> {
        let press = self.active.get_mut(&id)?;
        press.position = position;
        press.dragged |= press.origin.distance(position) >= 12.0;
        press.dragged.then_some(PointerGesture {
            id,
            kind: PointerKind::Drag,
            position,
            delta: position - press.origin,
        })
    }
    /// Emit one long-press after 0.5 seconds without dragging.
    pub fn holds(&mut self, seconds: f64) -> Vec<PointerGesture> {
        self.active
            .iter_mut()
            .filter_map(|(&id, press)| {
                if !press.dragged && !press.held && seconds - press.start >= 0.5 {
                    press.held = true;
                    Some(PointerGesture {
                        id,
                        kind: PointerKind::LongPress,
                        position: press.position,
                        delta: press.position - press.origin,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
    /// Release a press. Drags and previously emitted holds never become taps.
    pub fn end(&mut self, id: u64, position: Vec2, seconds: f64) -> Option<PointerGesture> {
        let press = self.active.remove(&id)?;
        if press.dragged || press.held || press.origin.distance(position) >= 12.0 {
            return None;
        }
        Some(PointerGesture {
            id,
            kind: if seconds - press.start >= 0.5 {
                PointerKind::LongPress
            } else {
                PointerKind::Tap
            },
            position,
            delta: position - press.origin,
        })
    }
    /// Cancel without generating a tap.
    pub fn cancel(&mut self, id: u64) {
        self.active.remove(&id);
    }
}

pub(crate) fn collect_input(
    mut touches: MessageReader<TouchInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    time: Res<Time>,
    mut tracker: ResMut<PointerTracker>,
    mut gestures: MessageWriter<PointerGesture>,
    mut unlocked: ResMut<crate::AudioUnlocked>,
) {
    use bevy::input::touch::TouchPhase;
    let seconds = time.elapsed_secs_f64();
    for touch in touches.read() {
        match touch.phase {
            TouchPhase::Started => {
                tracker.begin(touch.id, touch.position, seconds);
                unlocked.0 = true;
            }
            TouchPhase::Moved => {
                if let Some(g) = tracker.movement(touch.id, touch.position) {
                    gestures.write(g);
                }
            }
            TouchPhase::Ended => {
                if let Some(g) = tracker.end(touch.id, touch.position, seconds) {
                    gestures.write(g);
                }
            }
            TouchPhase::Canceled => tracker.cancel(touch.id),
        }
    }
    if let Ok(window) = windows.single() {
        if let Some(position) = window.cursor_position() {
            if mouse.just_pressed(MouseButton::Left) {
                tracker.begin(u64::MAX, position, seconds);
                unlocked.0 = true;
            }
            if mouse.pressed(MouseButton::Left)
                && let Some(g) = tracker.movement(u64::MAX, position)
            {
                gestures.write(g);
            }
            if mouse.just_released(MouseButton::Left)
                && let Some(g) = tracker.end(u64::MAX, position, seconds)
            {
                gestures.write(g);
            }
        } else if mouse.just_released(MouseButton::Left) {
            tracker.cancel(u64::MAX);
        }
    }
    for gesture in tracker.holds(seconds) {
        gestures.write(gesture);
    }
}
