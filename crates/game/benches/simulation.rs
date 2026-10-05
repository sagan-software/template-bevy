//! Criterion benchmarks for the deterministic simulation kernel.
//!
//! These benchmarks call the same pure functions used by prediction and
//! authority. They deliberately avoid windows, rendering, networking, and wall
//! clocks so a saved Criterion baseline measures gameplay work rather than
//! machine-dependent application startup.

use bevy::prelude::*;
use criterion::BatchSize;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use game::MovementStep;
use game::movement_velocity;
use game::reflect_at_bounds;
use std::hint::black_box;

/// Configures Criterion from command-line arguments and runs every group.
fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    benchmark_boundary_batch(&mut criterion);
    benchmark_warmed_schedule(&mut criterion);
    criterion.final_summary();
}

/// Measures a representative batch of pure arena-boundary calculations.
fn benchmark_boundary_batch(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("simulation/pure_boundary_batch");
    group.throughput(Throughput::Elements(BOUNDARY_SAMPLE_COUNT));
    group.bench_function("six_mixed_samples", |bencher| {
        // `iter_batched` keeps fixture creation outside the timed section. The
        // array is tiny, but this pattern scales cleanly to stateful kernels.
        bencher.iter_batched(
            boundary_samples,
            |samples| {
                let mut resolved_velocity = Vec2::ZERO;
                let mut direction_changes = 0_u32;

                for sample in samples {
                    let result = reflect_at_bounds(black_box(sample));
                    resolved_velocity += result.velocity();
                    direction_changes =
                        direction_changes.saturating_add(u32::from(result.is_direction_changed()));
                }

                black_box((resolved_velocity, direction_changes))
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// Builds neutral, interior, wall, and corner samples for one measured batch.
fn boundary_samples() -> [MovementStep; BOUNDARY_SAMPLE_COUNT_USIZE] {
    [
        MovementStep::new(Vec2::ZERO, Vec2::ZERO, BODY_HALF_EXTENT),
        MovementStep::new(Vec2::ZERO, Vec2::new(80.0, 40.0), BODY_HALF_EXTENT),
        MovementStep::new(
            Vec2::new(ARENA_X_LIMIT, 0.0),
            Vec2::new(80.0, 40.0),
            BODY_HALF_EXTENT,
        ),
        MovementStep::new(
            Vec2::new(-ARENA_X_LIMIT, 0.0),
            Vec2::new(-80.0, 40.0),
            BODY_HALF_EXTENT,
        ),
        MovementStep::new(
            Vec2::new(0.0, ARENA_Y_LIMIT),
            Vec2::new(80.0, 40.0),
            BODY_HALF_EXTENT,
        ),
        MovementStep::new(
            Vec2::new(ARENA_X_LIMIT, ARENA_Y_LIMIT),
            Vec2::new(80.0, 40.0),
            BODY_HALF_EXTENT,
        ),
    ]
}

/// Measures a warmed Bevy schedule at small and representative entity counts.
fn benchmark_warmed_schedule(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("simulation/warmed_ecs_schedule");

    for entity_count in SCHEDULE_ENTITY_COUNTS {
        let (mut world, mut schedule) = build_schedule_case(entity_count);
        group.throughput(Throughput::Elements(
            u64::try_from(entity_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::from_parameter(entity_count),
            &entity_count,
            |bencher, _| {
                bencher.iter(|| {
                    // The World and Schedule are constructed and warmed once.
                    // The measured section therefore captures steady-state ECS
                    // dispatch, query iteration, and the production kernel.
                    schedule.run(&mut world);
                    black_box(world.entities().len())
                });
            },
        );
    }

    group.finish();
}

/// Constructs a stable World and warms its query/archetype caches once.
fn build_schedule_case(entity_count: usize) -> (World, Schedule) {
    let mut world = World::new();

    for index in 0..entity_count {
        let (position, direction, is_boosting) = match index % 4 {
            0 => (Vec2::ZERO, Vec2::ZERO, false),
            1 => (Vec2::ZERO, Vec2::X, false),
            2 => (Vec2::ZERO, Vec2::ONE, true),
            _ => (Vec2::new(ARENA_X_LIMIT, 0.0), Vec2::X, true),
        };
        world.spawn(ScheduleBody {
            position,
            direction,
            is_boosting,
            resolved_velocity: Vec2::ZERO,
        });
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(resolve_schedule_bodies);

    // Warming once avoids charging Criterion for one-time archetype and system
    // initialization that a running game has already paid before steady state.
    schedule.run(&mut world);
    (world, schedule)
}

/// Applies semantic movement and arena response to benchmark-only ECS storage.
fn resolve_schedule_bodies(mut bodies: Query<'_, '_, &mut ScheduleBody>) {
    for mut body in &mut bodies {
        let intended_velocity = movement_velocity(body.direction, body.is_boosting);
        let result = reflect_at_bounds(MovementStep::new(
            body.position,
            intended_velocity,
            BODY_HALF_EXTENT,
        ));
        body.resolved_velocity = result.velocity();
    }
}

/// Minimal ECS storage used to measure production simulation through a
/// schedule.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
struct ScheduleBody {
    /// Current body center in world-space pixels.
    position: Vec2,
    /// Semantic movement direction before speed policy is applied.
    direction: Vec2,
    /// Whether the semantic Boost action is held.
    is_boosting: bool,
    /// Velocity produced by the most recent warmed schedule tick.
    resolved_velocity: Vec2,
}

/// Collision half-size shared by every synthetic benchmark body.
const BODY_HALF_EXTENT: Vec2 = Vec2::splat(24.0);
/// Furthest legal x coordinate for a body with [`BODY_HALF_EXTENT`].
const ARENA_X_LIMIT: f32 = 336.0;
/// Furthest legal y coordinate for a body with [`BODY_HALF_EXTENT`].
const ARENA_Y_LIMIT: f32 = 196.0;
/// Number of boundary samples processed by one pure-kernel iteration.
const BOUNDARY_SAMPLE_COUNT_USIZE: usize = 6;
/// Criterion throughput form of [`BOUNDARY_SAMPLE_COUNT_USIZE`].
const BOUNDARY_SAMPLE_COUNT: u64 = 6;
/// Entity counts representing a small scene and a busier gameplay workload.
const SCHEDULE_ENTITY_COUNTS: [usize; 2] = [64, 1_024];
