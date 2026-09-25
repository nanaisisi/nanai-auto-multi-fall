pub mod aerial_search;
pub mod candidates;
pub mod spawn_search;

pub use aerial_search::find_best_aerial_move;
#[allow(unused_imports)]
pub use candidates::{determine_autonomous_pace, generate_aerial_candidates, generate_spawn_candidates};
pub use spawn_search::find_best_spawn_move;

use crate::ai::eval::Evaluator;
use crate::ai::pathfinding::PathFinder;
use crate::ai::types::{MoveEvaluation, PredictedPlacement, ScoreBreakdown};
use crate::board::{CompactBoard, GlobalBoard};
use crate::game::LaneSignalBoard;
use crate::tromino::TrominoKind;

pub struct AutoAi;

impl AutoAi {
    /// 特定レーンの投入口からスポーンされたトミノに対する最適配置手を探索（タックイン・スライド対応）
    #[inline]
    pub fn find_best_move(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        predicted_others: &[PredictedPlacement],
        air_obstacles: &[(i32, i32)],
        signals: &LaneSignalBoard,
    ) -> Option<MoveEvaluation> {
        find_best_spawn_move(board, lane_id, kind, predicted_others, air_obstacles, signals)
    }

    /// 落下中の空中現在位置から再計算を行うメソッド（現在の予定に対するコミットメント維持ボーナス対応）
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn find_best_move_from_position(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        current_x: i32,
        current_y: i32,
        current_rotation: usize,
        current_target: Option<(i32, usize)>,
        predicted_others: &[PredictedPlacement],
        air_obstacles: &[(i32, i32)],
        signals: &LaneSignalBoard,
    ) -> Option<MoveEvaluation> {
        find_best_aerial_move(
            board,
            lane_id,
            kind,
            current_x,
            current_y,
            current_rotation,
            current_target,
            predicted_others,
            air_obstacles,
            signals,
        )
    }

    // --- 互換用委譲メソッド ---
    #[allow(dead_code, clippy::too_many_arguments)]
    #[inline(always)]
    pub fn simulate_drop_from_path(
        board: &GlobalBoard,
        kind: &TrominoKind,
        from_rot: usize,
        to_rot: usize,
        start_x: i32,
        start_y: i32,
        target_x: i32,
        reserved_landing_cells: &[(i32, i32)],
    ) -> Option<(i32, Vec<(i32, i32)>)> {
        PathFinder::simulate_drop_from_path(
            board,
            kind,
            from_rot,
            to_rot,
            start_x,
            start_y,
            target_x,
            reserved_landing_cells,
        )
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn simulate_drop_with_tuck_path(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        rot: usize,
        target_x: i32,
        reserved_landing_cells: &[(i32, i32)],
        air_obstacles: &[(i32, i32)],
    ) -> Option<(i32, Vec<(i32, i32)>)> {
        PathFinder::simulate_drop_with_tuck_path(
            board,
            lane_id,
            kind,
            rot,
            target_x,
            reserved_landing_cells,
            air_obstacles,
        )
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn simulate_drop_with_tuck(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        rot: usize,
        target_x: i32,
        reserved_landing_cells: &[(i32, i32)],
        air_obstacles: &[(i32, i32)],
    ) -> Option<i32> {
        PathFinder::simulate_drop_with_tuck(
            board,
            lane_id,
            kind,
            rot,
            target_x,
            reserved_landing_cells,
            air_obstacles,
        )
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    #[inline(always)]
    pub fn evaluate_placement_compact(
        current_board: &CompactBoard,
        future_board: &CompactBoard,
        player_id: usize,
        kind: &TrominoKind,
        rot: usize,
        x: i32,
        y: i32,
        predicted_others: &[PredictedPlacement],
        signals: &LaneSignalBoard,
    ) -> (f32, ScoreBreakdown) {
        Evaluator::evaluate_placement_compact(
            current_board,
            future_board,
            player_id,
            kind,
            rot,
            x,
            y,
            predicted_others,
            signals,
        )
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    #[inline(always)]
    pub fn evaluate_placement(
        current_board: &GlobalBoard,
        future_board: &GlobalBoard,
        player_id: usize,
        kind: &TrominoKind,
        rot: usize,
        x: i32,
        y: i32,
        predicted_others: &[PredictedPlacement],
        signals: &LaneSignalBoard,
    ) -> f32 {
        Evaluator::evaluate_placement(
            current_board,
            future_board,
            player_id,
            kind,
            rot,
            x,
            y,
            predicted_others,
            signals,
        )
    }
}
