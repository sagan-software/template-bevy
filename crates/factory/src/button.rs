//! Tap activation for screen buttons through the unified pointer adapter.
use crate::PointerGesture;
use crate::PointerKind;
use bevy::prelude::*;
use bevy::ui::CalculatedClip;
use bevy::ui::UiStack;
use bevy::ui_widgets::Activate;

/// Screen button activated on tap release. Drags and long presses never activate it.
#[derive(Component, Debug, Default)]
pub struct FactoryButton;

pub(crate) fn activate_buttons(
    mut gestures: MessageReader<PointerGesture>,
    stack: Res<UiStack>,
    buttons: Query<
        (&ComputedNode, &UiGlobalTransform, Option<&CalculatedClip>),
        With<FactoryButton>,
    >,
    mut commands: Commands,
) {
    for gesture in gestures
        .read()
        .filter(|gesture| gesture.kind == PointerKind::Tap)
    {
        for &entity in stack.uinodes.iter().rev() {
            if let Ok((node, transform, clip)) = buttons.get(entity) {
                let point = gesture.position / node.inverse_scale_factor;
                if node.size.min_element() > 0.0
                    && node.contains_point(*transform, point)
                    && clip.is_none_or(|clip| clip.contains_point(point))
                {
                    commands.trigger(Activate { entity });
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Resource, Default)]
    struct Activations(Vec<Entity>);

    /// Tap release uses physical layout bounds, skips hidden nodes and activates only the top button.
    #[test]
    fn taps_respect_layout_scale_and_gesture_kind() {
        let mut app = App::new();
        app.add_message::<PointerGesture>()
            .init_resource::<UiStack>()
            .init_resource::<Activations>()
            .add_systems(Update, activate_buttons)
            .add_observer(
                |event: On<Activate>, mut activations: ResMut<Activations>| {
                    activations.0.push(event.entity);
                },
            );
        let bottom = app
            .world_mut()
            .spawn((
                FactoryButton,
                ComputedNode {
                    size: Vec2::splat(80.0),
                    inverse_scale_factor: 0.5,
                    ..default()
                },
                UiGlobalTransform::from_translation(Vec2::splat(100.0)),
            ))
            .id();
        let top = app
            .world_mut()
            .spawn((
                FactoryButton,
                ComputedNode {
                    size: Vec2::splat(80.0),
                    inverse_scale_factor: 0.5,
                    ..default()
                },
                UiGlobalTransform::from_translation(Vec2::splat(100.0)),
            ))
            .id();
        let hidden = app
            .world_mut()
            .spawn((
                FactoryButton,
                ComputedNode::default(),
                UiGlobalTransform::default(),
            ))
            .id();
        app.world_mut().resource_mut::<UiStack>().uinodes = vec![bottom, top, hidden];
        for (kind, position) in [
            (PointerKind::Drag, Vec2::splat(50.0)),
            (PointerKind::LongPress, Vec2::splat(50.0)),
            (PointerKind::Tap, Vec2::ZERO),
            (PointerKind::Tap, Vec2::splat(50.0)),
        ] {
            app.world_mut().write_message(PointerGesture {
                id: 1,
                kind,
                position,
                delta: Vec2::ZERO,
            });
        }
        app.update();
        assert_eq!(app.world().resource::<Activations>().0, vec![top]);
        app.world_mut()
            .entity_mut(top)
            .insert(CalculatedClip::FullyClipped);
        app.world_mut().write_message(PointerGesture {
            id: 1,
            kind: PointerKind::Tap,
            position: Vec2::splat(50.0),
            delta: Vec2::ZERO,
        });
        app.update();
        assert_eq!(app.world().resource::<Activations>().0, vec![top, bottom]);
    }
    /// Press and release in one frame still produce one activation for both mouse and touch.
    #[test]
    fn fast_mouse_and_touch_taps_activate() {
        use bevy::input::ButtonState;
        use bevy::input::mouse::MouseButtonInput;
        use bevy::input::touch::TouchPhase;
        let mut app = App::new();
        app.add_plugins(bevy::input::InputPlugin)
            .init_resource::<Time>()
            .add_plugins(crate::FactoryPlugin::default())
            .init_resource::<Activations>()
            .add_observer(
                |event: On<Activate>, mut activations: ResMut<Activations>| {
                    activations.0.push(event.entity);
                },
            );
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::splat(50.0)));
        let window = app.world_mut().spawn(window).id();
        let button = app
            .world_mut()
            .spawn((
                FactoryButton,
                ComputedNode {
                    size: Vec2::splat(80.0),
                    ..default()
                },
                UiGlobalTransform::from_translation(Vec2::splat(50.0)),
            ))
            .id();
        app.world_mut().resource_mut::<UiStack>().uinodes = vec![button];
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut().write_message(MouseButtonInput {
                button: MouseButton::Left,
                state,
                window,
            });
        }
        app.update();
        assert_eq!(app.world().resource::<Activations>().0, vec![button]);
        for phase in [TouchPhase::Started, TouchPhase::Ended] {
            app.world_mut().write_message(TouchInput {
                phase,
                position: Vec2::splat(50.0),
                window,
                force: None,
                id: 42,
            });
        }
        app.update();
        assert_eq!(
            app.world().resource::<Activations>().0,
            vec![button, button]
        );
    }
}
