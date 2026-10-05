//! Fits planar starter scenes into landscape and portrait viewports.

use bevy::camera::ScalingMode;
use bevy::prelude::OrthographicProjection;

/// Keeps the complete arena and its margin visible at every aspect ratio.
pub(crate) fn planar_projection() -> OrthographicProjection {
    // Maintain the arena margin instead of cropping world coordinates on portrait screens.
    let mut projection = OrthographicProjection::default_2d();
    projection.scaling_mode = ScalingMode::AutoMin {
        min_width: 800.0,
        min_height: 500.0,
    };
    projection
}

/// Verifies world-space coverage after each supported viewport resize.
#[cfg(test)]
mod tests {
    use super::planar_projection;
    use bevy::camera::CameraProjection;

    /// Covers desktop, phone portrait, and a narrow split-screen viewport.
    #[test]
    fn arena_remains_visible_after_resizing() {
        for (width, height) in [(960.0, 600.0), (390.0, 844.0), (480.0, 600.0)] {
            let mut projection = planar_projection();
            projection.update(width, height);
            assert!(projection.area.width() >= 800.0);
            assert!(projection.area.height() >= 500.0);
        }
    }
}
