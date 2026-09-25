use crate::ai::{AutoAi, PredictedPlacement};
use crate::board::{GlobalBoard, lane_x_range};
use crate::config::{GameSettings, LANE_HEIGHT, LOCK_DELAY, MAX_LOCK_RESETS};
use crate::game::{
    FallingTromino, GameLogger, LanePace, LaneSignalBoard, LaneSlot, LaneSpawnCooldown,
};
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
        // 全員積み（ゲームオーバー）時はスポーンを停止
        return;
    }

    let Ok(mut rng) = rng_query.single_mut() else {
        return;
    };

    let base_interval = settings.current_base_fall_interval(board.lines_cleared);

    // 各落下中トミノの残り着地ステップ数（ETA）を算出して着地予測順にソート
    let mut falling_etas: Vec<(f32, PredictedPlacement)> = falling_query
        .iter()
        .map(|falling| {
            let dy = (falling.current_y - falling.landing_y as f32).max(0.0);
            let interval = match falling.pace {
                LanePace::SoftDrop => base_interval * settings.soft_drop_multiplier,
                LanePace::Normal => base_interval,
            };
            let eta = dy * interval;
            (
                eta,
                PredictedPlacement {
                    player_id: falling.lane_id,
                    kind: falling.kind,
                    rotation: falling.target_rotation,
                    target_x: falling.target_x,
                    landing_y: falling.landing_y,
                },
            )
        })
        .collect();

    falling_etas.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut predicted_others: Vec<PredictedPlacement> =
        falling_etas.into_iter().map(|(_, p)| p).collect();

    let mut air_obstacles: Vec<(i32, i32)> = Vec::new();
    for falling in falling_query.iter() {
        let offsets = falling.kind.cell_offsets(falling.current_rotation);
        for (dx, dy) in &offsets {
            air_obstacles.push((
                falling.current_x.round() as i32 + dx,
                falling.current_y.round() as i32 + dy,
            ));
        }
    }

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

        let (lane_min_x, _) = lane_x_range(lane.id);
        let start_x = lane_min_x as f32;
        let start_y = (LANE_HEIGHT - 1) as f32;

        let kind = TrominoKind::random_from_rng(&mut rng);

        // スポーン時の初期姿勢
        let initial_rotation = match kind {
            TrominoKind::Straight => 1,
            TrominoKind::Corner => 0,
        };

        // 投入口直下にブロックが詰まっているかチェック
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
                // スポーン成功 -> 当該レーンの積み状態は解消（回復）
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
                let fall_interval = match lane_pace {
                    LanePace::SoftDrop => base_interval * settings.soft_drop_multiplier,
                    LanePace::Normal => base_interval,
                };

                commands.spawn(FallingTromino {
                    lane_id: lane.id,
                    kind,
                    current_rotation: initial_rotation,
                    target_rotation: m.rotation,
                    current_x: start_x,
                    current_y: start_y,
                    target_x: m.target_x,
                    landing_y: m.landing_y,
                    fall_timer: Timer::from_seconds(fall_interval, TimerMode::Repeating),
                    lock_timer: Timer::from_seconds(LOCK_DELAY, TimerMode::Once),
                    rotate_timer: Timer::from_seconds(0.08, TimerMode::Repeating),
                    is_on_ground: false,
                    lock_resets_left: MAX_LOCK_RESETS,
                    pace: lane_pace,
                    waypoints: m.waypoints,
                    planned_board_version: board.board_version,
                    replan_timer: Timer::from_seconds(0.60, TimerMode::Repeating),
                });
            }
            None => {
                // 次の形が出せない（一時的な積み）状態
                let heights = board.column_heights();
                let max_h = heights.iter().max().copied().unwrap_or(0);
                let holes = board.count_holes();
                let (lane_min_x, lane_max_x) = lane_x_range(lane.id);
                let lane_max_h = (lane_min_x..=lane_max_x)
                    .map(|x| heights[x])
                    .max()
                    .unwrap_or(0);

                board.lane_stuck[lane.id] = true;

                if !can_enter_spawn {
                    let stuck_msg = format!(
                        "[LANE STUCK] Lane {} entrance blocked! (LaneMaxH: {}, GlobalMaxH: {}, Holes: {})",
                        lane.id + 1,
                        lane_max_h,
                        max_h,
                        holes
                    );
                    warn!("{}", stuck_msg);
                    logger.log(&stuck_msg);
                } else {
                    let no_path_msg = format!(
                        "[LANE STUCK] Lane {} no valid path/placement found for {:?} (LaneMaxH: {}, Holes: {})",
                        lane.id + 1,
                        kind,
                        lane_max_h,
                        holes
                    );
                    warn!("{}", no_path_msg);
                    logger.log(&no_path_msg);
                }

                if board.lane_stuck.iter().all(|&stuck| stuck) {
                    board.game_over = true;
                    let game_over_msg = format!(
                        "[GAME OVER] All lanes stuck! Final Lines: {}, Score: {}, Holes: {}, MaxH: {}",
                        board.lines_cleared, board.score, holes, max_h
                    );
                    error!("{}", game_over_msg);
                    logger.log(&game_over_msg);
                    logger.log_raw(&board.render_ascii());
                } else {
                    logger.log_raw(&board.render_ascii());
                }

                commands.entity(lane_entity).insert(LaneSpawnCooldown {
                    timer: Timer::from_seconds(0.25, TimerMode::Once),
                });
            }
        }
    }
}
