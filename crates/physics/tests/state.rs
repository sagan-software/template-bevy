//! Physics state contracts independent of rendered gameplay.

use bevy::prelude::Vec2;
use game_physics::rectangle;
use game_physics::segment;

/// Keeps planar collider geometry consistent in each supported backend.
#[test]
fn arena_shapes_preserve_coordinates_and_half_extents() {
    let start = Vec2::new(-3.0, 4.0);
    let end = Vec2::new(5.0, -6.0);
    let edge = segment(start, end);
    let edge = edge.as_segment().expect("arena edge is a segment");
    let half_extent = Vec2::new(7.0, 8.0);
    let wall = rectangle(half_extent);
    let wall = wall.as_cuboid().expect("arena wall is a cuboid");
    #[cfg(feature = "dim2")]
    {
        assert_eq!(edge.a(), start);
        assert_eq!(edge.b(), end);
        assert_eq!(wall.half_extents(), half_extent);
    }
    #[cfg(all(feature = "dim3", not(feature = "dim2")))]
    {
        assert_eq!(edge.a(), start.extend(0.0));
        assert_eq!(edge.b(), end.extend(0.0));
        assert_eq!(wall.half_extents(), half_extent.extend(half_extent.y));
    }
}

/// Verifies corrections across translated positions and wrapped orientations.
#[cfg(feature = "networked")]
#[test]
fn network_corrections_restore_the_received_state() {
    use bevy::prelude::Rot2;
    use game_physics::Position;
    use game_physics::Rotation;
    use lightyear::prelude::Diffable;

    assert_eq!(
        <Position as Diffable<Vec2>>::base_value(),
        Position(Vec2::ZERO)
    );
    assert_eq!(
        <Rotation as Diffable<Rot2>>::base_value(),
        Rotation::default()
    );
    let mut old = Position(Vec2::new(-4.0, 8.0));
    let received = Position(Vec2::new(9.0, -2.0));
    let correction = old.diff(&received);
    old.apply_diff(&correction);
    assert_eq!(old, received);
    for (before, after) in [(0.0, 0.5), (3.0, -3.0), (-3.0, 3.0)] {
        let mut old = Rotation::radians(before);
        let received = Rotation::radians(after);
        let correction = old.diff(&received);
        old.apply_diff(&correction);
        assert!(old.angle_between(received).abs() < 0.00001);
    }
}
