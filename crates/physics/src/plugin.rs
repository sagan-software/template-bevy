//! Composes the selected Rapier backend and replicated state adapters.

use crate::AngularVelocity;
use crate::LinearVelocity;
use crate::Position;
use crate::Rotation;
use crate::backend::NoUserData;
use crate::backend::PhysicsSet;
use crate::backend::RapierConfiguration;
use crate::backend::RapierContextInitialization;
use crate::backend::RapierPhysicsPlugin;
use crate::backend::RigidBody;
use crate::backend::TimestepMode;
use crate::backend::Velocity;
use crate::dimension::IntegrationParameters;
use crate::dimension::PhysicsVector;
use bevy::prelude::*;
use game_settings::TickRate;

/// Installs Rapier on the fixed simulation schedule and synchronizes planar
/// gameplay components. Add this plugin after Bevy transform support, with a
/// validated tick rate.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsPlugin {
    /// Validated frequency that determines the integration step.
    tick_rate: TickRate,
}

impl PhysicsPlugin {
    /// Selects the validated frequency used to derive the Rapier integration
    /// period. Use the same rate for gameplay and networking before adding this
    /// plugin to an application.
    #[must_use]
    pub const fn with_tick_rate(mut self, tick_rate: TickRate) -> Self {
        self.tick_rate = tick_rate;
        self
    }
}

/// Orders gameplay state synchronization around the Rapier integration step.
///
/// Schedule movement before preparation and consume completed physics state
/// after capture in the fixed-update schedule.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum PhysicsSystems {
    /// Copies retained gameplay position, rotation, and velocity into Rapier
    /// before backend synchronization. Systems changing those components must
    /// run before this set in fixed updates.
    Prepare,
    /// Copies Rapier's completed integration into retained gameplay components
    /// after backend writeback.
    ///
    /// Equal values remain unchanged so networking and other consumers avoid
    /// false change notifications.
    Capture,
}

impl Plugin for PhysicsPlugin {
    /// Adds a gravity-free backend and explicit state synchronization.
    fn build(&self, app: &mut App) {
        // A length unit of 100 world units preserves pixel-scale contact tolerances.
        let initialization = RapierContextInitialization::InitializeDefaultRapierContext {
            integration_parameters: IntegrationParameters {
                length_unit: 100.0,
                ..default()
            },
            rapier_configuration: RapierConfiguration {
                gravity: PhysicsVector::ZERO,
                force_update_from_transform_changes: true,
                ..RapierConfiguration::new(100.0)
            },
        };
        app.insert_resource(TimestepMode::Fixed {
            dt: self.tick_rate.period().as_secs_f32(),
            substeps: 1,
        })
        .add_plugins(
            RapierPhysicsPlugin::<NoUserData>::default()
                .with_custom_initialization(initialization)
                .in_fixed_schedule(),
        )
        .configure_sets(
            FixedUpdate,
            PhysicsSystems::Prepare.before(PhysicsSet::SyncBackend),
        )
        .configure_sets(
            FixedUpdate,
            PhysicsSystems::Capture.after(PhysicsSet::Writeback),
        )
        .add_systems(FixedUpdate, prepare_bodies.in_set(PhysicsSystems::Prepare))
        .add_systems(FixedUpdate, capture_bodies.in_set(PhysicsSystems::Capture));
    }
}

/// Applies authoritative or predicted state before each integration step.
fn prepare_bodies(
    mut commands: Commands<'_, '_>,
    mut bodies: Query<
        '_,
        '_,
        (
            Entity,
            &Position,
            &Rotation,
            &LinearVelocity,
            Option<&AngularVelocity>,
            &mut Transform,
        ),
        With<RigidBody>,
    >,
) {
    // Copy retained gameplay state before Rapier synchronizes its backend.
    for (entity, position, rotation, velocity, angular, mut transform) in &mut bodies {
        // Derive transforms from planar state instead of storing a second position.
        transform.translation = position.0.extend(0.0);
        transform.rotation = Quat::from_rotation_z(rotation.0.as_radians());
        let angular = angular.map_or(0.0, |value| value.0);
        // The selected adapter adds planar axis locks only for the 3D backend.
        commands
            .entity(entity)
            .insert(crate::dimension::velocity(velocity.0, angular));
    }
}

/// Retains completed physics state without marking equal values changed.
fn capture_bodies(
    mut bodies: Query<
        '_,
        '_,
        (
            &Transform,
            &Velocity,
            &mut Position,
            &mut Rotation,
            &mut LinearVelocity,
            Option<&mut AngularVelocity>,
        ),
        With<RigidBody>,
    >,
) {
    // Capture completed integration without marking unchanged values as modified.
    for (transform, velocity, mut position, mut rotation, mut linear, angular) in &mut bodies {
        position.set_if_neq(Position(transform.translation.truncate()));
        let (_, _, angle) = transform.rotation.to_euler(EulerRot::XYZ);
        rotation.set_if_neq(Rotation::radians(angle));
        // Read the common XY coordinates before converting backend angular motion.
        linear.set_if_neq(LinearVelocity(Vec2::new(
            velocity.linear.x,
            velocity.linear.y,
        )));
        if let Some(mut angular) = angular {
            angular.set_if_neq(AngularVelocity(crate::dimension::angular_velocity(
                velocity,
            )));
        }
    }
}

/// Tests private state ingress, capture, and configured integration timing.
#[cfg(test)]
mod tests {
    use super::*;

    /// Preserves the selected fixed step and neutral gravity configuration.
    #[test]
    fn configured_rate_controls_rapier_step() {
        let mut app = App::new();
        let rate = TickRate::new(30).expect("30 Hz is supported");
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugin::default().with_tick_rate(rate),
        ));
        assert!(
            matches!(*app.world().resource::<TimestepMode>(), TimestepMode::Fixed { dt, substeps: 1 } if (dt - 1.0 / 30.0).abs() < f32::EPSILON)
        );
    }

    /// Copies complete state and supplies zero angular speed when absent.
    #[test]
    fn prepare_copies_optional_angular_state() {
        let mut app = App::new();
        app.add_systems(Update, prepare_bodies);
        let entity = app
            .world_mut()
            .spawn((
                RigidBody::Dynamic,
                Position(Vec2::new(3.0, 4.0)),
                Rotation::radians(0.5),
                LinearVelocity(Vec2::new(5.0, 6.0)),
            ))
            .id();
        app.update();
        let transform = app
            .world()
            .get::<Transform>(entity)
            .expect("position requires a transform");
        assert_eq!(transform.translation, Vec3::new(3.0, 4.0, 0.0));
        let velocity = app
            .world()
            .get::<Velocity>(entity)
            .expect("physics velocity is installed");
        #[cfg(feature = "dim2")]
        {
            assert_eq!(velocity.linear, Vec2::new(5.0, 6.0));
            assert_eq!(velocity.angular, 0.0);
        }
        #[cfg(all(feature = "dim3", not(feature = "dim2")))]
        {
            assert_eq!(velocity.linear, Vec3::new(5.0, 6.0, 0.0));
            assert_eq!(velocity.angular, Vec3::ZERO);
        }
    }
}
