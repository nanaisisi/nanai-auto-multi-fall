use crate::ai::{AutoAi, PredictedPlacement};
use crate::board::GlobalBoard;
use crate::config::GameSettings;
use crate::game::{FallingTromino, GameLogger, LanePace, LaneSignalBoard};
use bevy::prelude::*;

/// トミノ落下中の目標再計算（Dynamic Re-planning）
#[allow(clippy::too_many_arguments)]
pub fn update_replan_if_needed(
    falling: &mut FallingTromino,
    board: &GlobalBoard,
    all_predicted_others: &[PredictedPlacement],
    all_air_obstacles: &[(i32, i32)],
    signals: &LaneSignalBoard,
    settings: &GameSettings,
    base_interval: f32,
    logger: &mut GameLogger,
    current_int_x: i32,
    current_int_y: i32,
    delta: std::time::Duration,
) {
    falling.replan_timer.tick(delta);

    let target_invalidated = !board.can_place(
        &falling.kind,
        falling.target_rotation,
        falling.target_x,
        falling.landing_y,
    );

    let board_changed = falling.planned_board_version != board.board_version;
    let should_replan = !falling.is_on_ground
        && (target_invalidated || board_changed || falling.replan_timer.just_finished());

    if should_replan {
        let this_others: Vec<PredictedPlacement> = all_predicted_others
            .iter()
            .filter(|p| p.player_id != falling.lane_id)
            .cloned()
            .collect();

        if let Some(re_eval) = AutoAi::find_best_move_from_position(
            board,
            falling.lane_id,
            &falling.kind,
            current_int_x,
            current_int_y,
            falling.current_rotation,
            Some((falling.target_x, falling.target_rotation)),
            &this_others,
            all_air_obstacles,
            signals,
        ) {
            if falling.target_x != re_eval.target_x
                || falling.landing_y != re_eval.landing_y
                || falling.target_rotation != re_eval.rotation
            {
                let replan_msg = format!(
                    "[AI Lane {}] Replan shift: (x:{}, y:{}, rot:{}) -> (x:{}, y:{}, rot:{}) | Breakdown: {}",
                    falling.lane_id + 1,
                    falling.target_x,
                    falling.landing_y,
                    falling.target_rotation,
                    re_eval.target_x,
                    re_eval.landing_y,
                    re_eval.rotation,
                    re_eval.breakdown,
                );
                trace!("{}", replan_msg);
                logger.log(&replan_msg);
            }

            falling.target_x = re_eval.target_x;
            falling.landing_y = re_eval.landing_y;
            falling.target_rotation = re_eval.rotation;
            falling.waypoints = re_eval.waypoints;
            falling.pace = re_eval.pace;
            falling.planned_board_version = board.board_version;

            let fall_interval = match falling.pace {
                LanePace::SoftDrop => base_interval * settings.soft_drop_multiplier,
                LanePace::Normal => base_interval,
            };
            falling
                .fall_timer
                .set_duration(std::time::Duration::from_secs_f32(fall_interval));
        }
    }
}
