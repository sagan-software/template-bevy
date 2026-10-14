//! Insets and a UI root that follows them.
use bevy::prelude::*;

/// Safe-area insets in logical pixels. Native hosts may update this resource directly.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct SafeAreaInsets {
    /// Distance from the top edge.
    pub top: f32,
    /// Distance from the right edge.
    pub right: f32,
    /// Distance from the bottom edge.
    pub bottom: f32,
    /// Distance from the left edge.
    pub left: f32,
}
/// A root whose padding follows SafeAreaInsets.
#[derive(Component)]
pub struct SafeAreaRoot;

/// Create a fullscreen root. Add a child for the game's own spacing.
pub fn safe_area_root() -> impl Bundle {
    (
        SafeAreaRoot,
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
    )
}
/// A styled factory tap button. Attach an `On<Activate>` observer and a text child.
pub fn screen_button() -> impl Bundle {
    (
        crate::FactoryButton,
        Node {
            min_width: px(64),
            min_height: px(64),
            padding: UiRect::axes(px(24), px(12)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(255, 184, 92)),
    )
}
impl SafeAreaInsets {
    /// Normalize unavailable or invalid platform values to zero.
    pub fn sanitized(self) -> Self {
        fn valid(v: f32) -> f32 {
            if v.is_finite() { v.max(0.0) } else { 0.0 }
        }
        Self {
            top: valid(self.top),
            right: valid(self.right),
            bottom: valid(self.bottom),
            left: valid(self.left),
        }
    }
}
pub(crate) fn update_insets(
    mut insets: ResMut<SafeAreaInsets>,
    mut roots: Query<&mut Node, With<SafeAreaRoot>>,
) {
    #[cfg(target_arch = "wasm32")]
    if let Some(values) = browser_insets() {
        *insets = values;
    }
    #[cfg(target_os = "android")]
    if ANDROID_READY.load(std::sync::atomic::Ordering::Relaxed) {
        let values = ANDROID_INSETS
            .each_ref()
            .map(|v| f32::from_bits(v.load(std::sync::atomic::Ordering::Relaxed)));
        *insets = SafeAreaInsets {
            top: values[0],
            right: values[1],
            bottom: values[2],
            left: values[3],
        };
    }
    *insets = insets.sanitized();
    let values = *insets;
    for mut root in &mut roots {
        root.padding = UiRect {
            top: px(values.top),
            right: px(values.right),
            bottom: px(values.bottom),
            left: px(values.left),
        };
    }
}
#[cfg(target_arch = "wasm32")]
fn browser_insets() -> Option<SafeAreaInsets> {
    let window = web_sys::window()?;
    let probe = window.document()?.get_element_by_id("factory-safe-area")?;
    let style = window.get_computed_style(&probe).ok()??;
    let value = |name| {
        style
            .get_property_value(name)
            .ok()?
            .trim_end_matches("px")
            .parse()
            .ok()
    };
    Some(SafeAreaInsets {
        top: value("padding-top")?,
        right: value("padding-right")?,
        bottom: value("padding-bottom")?,
        left: value("padding-left")?,
    })
}
#[cfg(target_os = "android")]
static ANDROID_INSETS: [std::sync::atomic::AtomicU32; 4] =
    [const { std::sync::atomic::AtomicU32::new(0) }; 4];
#[cfg(target_os = "android")]
static ANDROID_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Android host bridge. Pass logical-pixel insets after each WindowInsets update.
#[cfg(target_os = "android")]
pub fn set_android_insets(top: f32, right: f32, bottom: f32, left: f32) {
    for (slot, value) in ANDROID_INSETS.iter().zip([top, right, bottom, left]) {
        slot.store(value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
    ANDROID_READY.store(true, std::sync::atomic::Ordering::Relaxed);
}
