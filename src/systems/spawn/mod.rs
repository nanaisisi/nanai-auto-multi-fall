pub mod prep;
pub mod stuck;

pub use prep::{collect_air_obstacles, collect_sorted_predictions};
pub use stuck::{LaneStuckContext, handle_lane_stuck};

use crate::ai::{AutoAi, PredictedPlacement};
use crate::board::GlobalBoard;
use crate::config::GameSettings;
use crate::game::{FallingTromino, GameLogger, LaneSignalBoard, LaneSlot, LaneSpawnCooldown};
use crate::tromino::TrominoKind;
use bevy::prelude::*;
use bevy_prng::WyRand;
use bevy_rand::prelude::GlobalRng;

/// 新規トミノのスポーンシステム（限定シグナル＆ペース連動対応）
#[allow(clippy::too_many_arguments)]
pub fn spawn_tromino_system(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut board: ResMut<GlobalBoard>,
    signals: Res<LaneSignalBoard>,
    mut logger: ResMut<GameLogger>,
    mut rng_query: Query<&mut WyRand, With<GlobalRng>>,
    mut lane_query: Query<(Entity, &LaneSlot, Option<&mut LaneSpawnCooldown>)>,
    falling_query: Query<&FallingTromino>,
) {
    if board.game_over {
        return;
    }

    let Ok(mut rng) = rng_query.single_mut() else {
        return;
    };

    let base_interval = settings.current_base_fall_interval(board.lines_cleared);
    let mut predicted_others =
        collect_sorted_predictions(&falling_query, &settings, board.lines_cleared);
    let air_obstacles = collect_air_obstacles(&falling_query);

    for (lane_entity, lane, cooldown_opt) in lane_query.iter_mut() {
        let has_active = falling_query.iter().any(|f| f.lane_id == lane.id);
        if has_active {
            continue;
        }

        if let Some(mut cooldown) = cooldown_opt {
            cooldown.timer.tick(time.delta());
            if !cooldown.timer.is_finished() {
                continue;
            }
            commands.entity(lane_entity).remove::<LaneSpawnCooldown>();
        }

        let (start_x, start_y) = GlobalBoard::lane_spawn_origin(lane.id);

        let kind = TrominoKind::random_from_rng(&mut rng);
        let initial_rotation = kind.initial_rotation();

        let can_enter_spawn =
            board.can_place(&kind, initial_rotation, start_x as i32, start_y as i32);

        let best_move = if can_enter_spawn {
            AutoAi::find_best_move(
                &board,
                lane.id,
                &kind,
                &predicted_others,
                &air_obstacles,
                &signals,
            )
        } else {
            None
        };

        match best_move {
            Some(m) => {
                board.lane_stuck[lane.id] = false;

                let spawn_msg = format!(
                    "[AI Lane {}] Spawning {:?} -> Target (x:{}, y:{}, rot:{}, pace:{:?}) | Breakdown: {}",
                    lane.id + 1,
                    kind,
                    m.target_x,
                    m.landing_y,
                    m.rotation,
                    m.pace,
                    m.breakdown,
                );
                debug!("{}", spawn_msg);
                logger.log(&spawn_msg);

                predicted_others.push(PredictedPlacement {
                    player_id: lane.id,
                    kind,
                    rotation: m.rotation,
                    target_x: m.target_x,
                    landing_y: m.landing_y,
                });

                let lane_pace = m.pace;
                let fall_interval = settings.calculate_fall_interval(base_interval, lane_pace);

                commands.spawn(FallingTromino::new_spawned(
                    lane.id,
                    kind,
                    initial_rotation,
                    m.rotation,
                    start_x,
                    start_y,
                    m.target_x,
                    m.landing_y,
                    fall_interval,
                    lane_pace,
                    m.waypoints,
                    board.board_version,
                ));
            }
            None => {
                handle_lane_stuck(
                    &mut commands,
                    &mut board,
                    LaneStuckContext {
                        lane_entity,
                        lane_id: lane.id,
                        kind,
                        can_enter_spawn,
                    },
                    &mut logger,
                );
            }
        }
    }
}
