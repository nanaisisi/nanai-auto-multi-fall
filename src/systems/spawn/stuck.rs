use crate::board::{GlobalBoard, lane_x_range};
use crate::game::GameLogger;
use crate::tromino::TrominoKind;
use bevy::prelude::*;

/// 投入口詰まりや配置候補なし時のスタック判定およびゲームオーバー判定・ログ出力
pub fn handle_lane_stuck(
    commands: &mut Commands,
    board: &mut GlobalBoard,
    lane_entity: Entity,
    lane_id: usize,
    kind: TrominoKind,
    can_enter_spawn: bool,
    logger: &mut GameLogger,
) {
    let heights = board.column_heights();
    let max_h = heights.iter().max().copied().unwrap_or(0);
    let holes = board.count_holes();
    let (lane_min_x, lane_max_x) = lane_x_range(lane_id);
    let lane_max_h = (lane_min_x..=lane_max_x)
        .map(|x| heights[x])
        .max()
        .unwrap_or(0);

    board.lane_stuck[lane_id] = true;

    if !can_enter_spawn {
        let stuck_msg = format!(
            "[LANE STUCK] Lane {} entrance blocked! (LaneMaxH: {}, GlobalMaxH: {}, Holes: {})",
            lane_id + 1,
            lane_max_h,
            max_h,
            holes
        );
        warn!("{}", stuck_msg);
        logger.log(&stuck_msg);
    } else {
        let no_path_msg = format!(
            "[LANE STUCK] Lane {} no valid path/placement found for {:?} (LaneMaxH: {}, Holes: {})",
            lane_id + 1,
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

    commands
        .entity(lane_entity)
        .insert(crate::game::LaneSpawnCooldown {
            timer: Timer::from_seconds(0.25, TimerMode::Once),
        });
}
