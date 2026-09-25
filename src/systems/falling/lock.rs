use crate::board::GlobalBoard;
use crate::config::GameSettings;
use crate::game::{FallingTromino, GameLogger, LanePace, LaneSlot, LaneSpawnCooldown};
use bevy::prelude::*;

/// トミノ接地後の固定（Lock）処理、ライン消去、クールダウン設定、デスポーン
#[allow(clippy::too_many_arguments)]
pub fn handle_lock_process(
    commands: &mut Commands,
    tromino_entity: Entity,
    falling: &FallingTromino,
    board: &mut GlobalBoard,
    logger: &mut GameLogger,
    settings: &GameSettings,
    lane_query: &Query<(Entity, &LaneSlot)>,
) {
    let lock_x = falling.current_x.round() as i32;
    let lock_y = falling.current_y.round() as i32;

    let safe_to_lock = board.can_place(&falling.kind, falling.current_rotation, lock_x, lock_y);

    if safe_to_lock {
        let locked = board.lock_tromino(
            falling.lane_id,
            &falling.kind,
            falling.current_rotation,
            lock_x,
            lock_y,
        );

        if locked {
            let lines = board.clear_full_lines();
            if lines > 0 {
                let clear_msg = format!(
                    "[LINE CLEAR] {} lines cleared by P{}! Total Lines: {}, Score: {}",
                    lines,
                    falling.lane_id + 1,
                    board.lines_cleared,
                    board.score
                );
                info!("{}", clear_msg);
                logger.log(&clear_msg);
                logger.log_raw(&board.render_ascii());
            } else {
                let lock_msg = format!(
                    "[LOCKED] P{} {:?} locked at (x:{}, y:{}, rot:{})",
                    falling.lane_id + 1,
                    falling.kind,
                    lock_x,
                    lock_y,
                    falling.current_rotation
                );
                debug!("{}", lock_msg);
                logger.log(&lock_msg);
            }
        } else {
            let heights = board.column_heights();
            let max_h = heights.iter().max().copied().unwrap_or(0);
            let overflow_msg = format!(
                "[TOP OVERFLOW] P{} locked above ceiling! (GlobalMaxH: {}, Lines: {})",
                falling.lane_id + 1,
                max_h,
                board.lines_cleared
            );
            warn!("{}", overflow_msg);
            logger.log(&overflow_msg);
            logger.log_raw(&board.render_ascii());
        }
    } else {
        let fail_msg = format!(
            "[LOCK FAILED] P{} {:?} cannot be placed at (x:{}, y:{}, rot:{})",
            falling.lane_id + 1,
            falling.kind,
            lock_x,
            lock_y,
            falling.current_rotation
        );
        warn!("{}", fail_msg);
        logger.log(&fail_msg);
    }

    for (lane_entity, lane) in lane_query.iter() {
        if lane.id == falling.lane_id {
            let pace_multiplier = match falling.pace {
                LanePace::SoftDrop => 0.65,
                LanePace::Normal => 1.0,
            };
            let delay = (settings.spawn_delay + (falling.lane_id as f32 * 0.04)) * pace_multiplier;

            commands.entity(lane_entity).insert(LaneSpawnCooldown {
                timer: Timer::from_seconds(delay, TimerMode::Once),
            });
            break;
        }
    }

    commands.entity(tromino_entity).despawn();
}
