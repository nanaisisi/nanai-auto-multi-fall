use crate::board::{GlobalBoard, lane_x_range};
use crate::game::{GameLogger, LaneSpawnCooldown};
use crate::tromino::TrominoKind;
use bevy::prelude::*;

/// スタック発生時のリトライ・クールダウン間隔（秒）
const STUCK_RETRY_INTERVAL_SECS: f32 = 0.25;

/// レーンスタック発生時のコンテキスト情報
pub struct LaneStuckContext {
    pub lane_entity: Entity,
    pub lane_id: usize,
    pub kind: TrominoKind,
    pub can_enter_spawn: bool,
}

/// 投入口詰まりや配置候補なし時のスタック判定およびゲームオーバー判定・ログ出力
pub fn handle_lane_stuck(
    commands: &mut Commands,
    board: &mut GlobalBoard,
    ctx: LaneStuckContext,
    logger: &mut GameLogger,
) {
    let heights = board.column_heights();
    let max_h = heights.iter().max().copied().unwrap_or(0);
    let holes = board.count_holes();
    let (lane_min_x, lane_max_x) = lane_x_range(ctx.lane_id);
    let lane_max_h = heights[lane_min_x..=lane_max_x]
        .iter()
        .max()
        .copied()
        .unwrap_or(0);

    let is_game_over = board.mark_lane_stuck(ctx.lane_id);

    let stuck_msg = if !ctx.can_enter_spawn {
        format!(
            "[LANE STUCK] Lane {} entrance blocked! (LaneMaxH: {}, GlobalMaxH: {}, Holes: {})",
            ctx.lane_id + 1,
            lane_max_h,
            max_h,
            holes
        )
    } else {
        format!(
            "[LANE STUCK] Lane {} no valid path/placement found for {:?} (LaneMaxH: {}, Holes: {})",
            ctx.lane_id + 1,
            ctx.kind,
            lane_max_h,
            holes
        )
    };
    warn!("{}", stuck_msg);
    logger.log(&stuck_msg);

    if is_game_over {
        let game_over_msg = format!(
            "[GAME OVER] All lanes stuck! Final Lines: {}, Score: {}, Holes: {}, MaxH: {}",
            board.lines_cleared, board.score, holes, max_h
        );
        error!("{}", game_over_msg);
        logger.log(&game_over_msg);
    }

    logger.log_raw(&board.render_ascii());

    commands.entity(ctx.lane_entity).insert(LaneSpawnCooldown {
        timer: Timer::from_seconds(STUCK_RETRY_INTERVAL_SECS, TimerMode::Once),
    });
}
