use bevy::prelude::*;
use bevy_prng::WyRand;
use bevy_rand::prelude::EntropyPlugin;

mod ai;
mod board;
mod config;
mod game;
mod render;
mod setup;
mod systems;
mod tromino;
mod ui;

#[cfg(test)]
mod tests;

fn main() {
    App::new()
        .insert_resource(config::GameSettings::default())
        .init_resource::<board::GlobalBoard>()
        .init_resource::<game::LaneSignalBoard>()
        .add_plugins(EntropyPlugin::<WyRand>::default())
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Nanai Auto Multi-Fall (Wide Connected Field)".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    filter: format!("{},icu_provider=error", bevy::log::DEFAULT_FILTER),
                    ..default()
                }),
        )
        .add_systems(Startup, (setup::setup_game, ui::setup_ui))
        .add_systems(
            Update,
            (
                systems::update_lane_signals_system,
                systems::spawn_tromino_system,
                systems::falling_tromino_system,
            ).chain(),
        )
        .add_systems(
            PostUpdate,
            (
                render::render_system,
                ui::update_ui_system,
            ),
        )

        .run();
}
