pub mod compact_eval;
pub mod cooperation;
pub mod global_eval;
pub mod metrics;
pub mod weights;

use crate::ai::types::{PredictedPlacement, ScoreBreakdown};
use crate::board::{CompactBoard, GlobalBoard};
use crate::game::LaneSignalBoard;
use crate::tromino::TrominoKind;

use compact_eval::*;
use cooperation::*;
use global_eval::*;
use metrics::*;
use weights::*;

pub struct Evaluator;

impl Evaluator {
    /// ビットマスク版CompactBoardを用いたゼロアロケーション・高速評価関数
    #[allow(clippy::too_many_arguments)]
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
        let mut sim_future = *future_board;
        sim_future.lock_tromino(kind, rot, x, y);
        let (future_lines, future_cleared_indices) = sim_future.clear_full_lines_with_indices();

        let mut sim_alone = *current_board;
        sim_alone.lock_tromino(kind, rot, x, y);
        let alone_lines = sim_alone.clear_full_lines();

        let cooperative_lines = future_lines.saturating_sub(alone_lines);

        let heights = sim_future.column_heights();
        let (sum_height, max_height, bumpiness) = calculate_height_metrics(&heights);
        let holes = sim_future.count_holes() as f32;

        let offsets = kind.cell_offsets(rot);
        let existing_border_h = if x >= 0 && (x as usize) < crate::config::TOTAL_GRID_WIDTH {
            current_board.column_heights()[x as usize]
        } else {
            0
        };
        let (border_bridge_bonus, border_barrier_penalty) =
            calculate_border_metrics(kind, rot, x, &offsets, existing_border_h);

        let adjacency_bonus = calculate_adjacency_bonus(&offsets, x, y, predicted_others);

        // 1. 空白フタ防止ペナルティ & 凹み・穴埋めボーナス
        let (anti_roof_penalty, hole_fill_bonus) =
            evaluate_roof_and_hole_compact(future_board, &sim_future, &offsets, x, y);

        // 2. 限定シグナルに基づく協調支援と配慮
        let mut active_vertical_wells: Vec<(usize, usize, usize, usize)> = Vec::new();
        for (wx, wy, wdepth) in future_board.find_lane_vertical_wells(player_id) {
            active_vertical_wells.push((player_id, wx, wy, wdepth));
        }
        for other_signal in &signals.signals {
            if other_signal.lane_id != player_id
                && let Some((wx, wy, wdepth)) = other_signal.vertical_well
            {
                active_vertical_wells.push((other_signal.lane_id, wx, wy, wdepth));
            }
        }

        let (well_cooperation_bonus, well_capping_penalty) = calculate_well_cooperation(
            player_id,
            kind,
            rot,
            x,
            y,
            &offsets,
            &active_vertical_wells,
        );

        let well_creation_penalty = evaluate_well_creation_compact(future_board, &sim_future);

        let (signal_cooperation_bonus, non_interference_penalty) =
            calculate_signal_cooperation(player_id, &offsets, x, signals);

        // 3. レーン中心からの距離ペナルティ
        let dist_penalty = calculate_lane_distance_penalty(player_id, x);

        // 4. 底のライン・低層配置優先
        let bottom_priority_bonus = calculate_bottom_priority_bonus(y);

        // 5. 穴・空白の上にあるラインの消去ボーナス
        let hole_clearance_bonus = if future_lines > 0 {
            evaluate_hole_clearance_compact(&sim_future, &future_cleared_indices)
        } else {
            0.0
        };

        // 6. ライン消去へ向けた行埋め進行度ボーナス
        let row_fill_bonus = evaluate_row_fill_compact(&sim_future, &offsets, y);

        let lines_val = future_lines as f32 * LINE_WEIGHT + hole_clearance_bonus + row_fill_bonus;
        let coop_lines_val = cooperative_lines as f32 * COOPERATIVE_BONUS_WEIGHT;
        let holes_val = holes * HOLES_WEIGHT_COMPACT;
        let height_val = (sum_height * HEIGHT_WEIGHT)
            + (max_height * MAX_HEIGHT_WEIGHT)
            + bottom_priority_bonus;
        let bumpiness_val = bumpiness * BUMPINESS_WEIGHT;
        let border_val = border_bridge_bonus + border_barrier_penalty;
        let well_val = well_cooperation_bonus + well_capping_penalty + well_creation_penalty;
        let sig_val =
            signal_cooperation_bonus + non_interference_penalty + adjacency_bonus + dist_penalty;

        let total = lines_val
            + coop_lines_val
            + border_val
            + adjacency_bonus
            + anti_roof_penalty
            + hole_fill_bonus
            + signal_cooperation_bonus
            + well_val
            + non_interference_penalty
            + dist_penalty
            + height_val
            + holes_val
            + bumpiness_val;

        let breakdown = ScoreBreakdown {
            total,
            lines: lines_val,
            coop_lines: coop_lines_val,
            holes_penalty: holes_val,
            height_penalty: height_val,
            bumpiness_penalty: bumpiness_val,
            anti_roof: anti_roof_penalty,
            hole_fill: hole_fill_bonus,
            border: border_val,
            signal_coop: sig_val,
            well_coop: well_val,
        };

        (total, breakdown)
    }

    /// 評価関数（空白フタ防止・人間的な曖昧シグナル支援・隣レーン不干渉を含む）
    #[allow(dead_code, clippy::too_many_arguments)]
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
        let mut sim_future = future_board.clone();
        sim_future.lock_tromino(player_id, kind, rot, x, y);
        let (future_lines, future_cleared_indices) = sim_future.clear_full_lines_with_indices();

        let mut sim_alone = current_board.clone();
        sim_alone.lock_tromino(player_id, kind, rot, x, y);
        let alone_lines = sim_alone.clear_full_lines();

        let cooperative_lines = future_lines.saturating_sub(alone_lines);

        let heights = sim_future.column_heights();
        let (sum_height, max_height, bumpiness) = calculate_height_metrics(&heights);
        let holes = sim_future.count_holes() as f32;

        let offsets = kind.cell_offsets(rot);
        let existing_border_h = if x >= 0 && (x as usize) < crate::config::TOTAL_GRID_WIDTH {
            current_board.column_heights()[x as usize]
        } else {
            0
        };
        let (border_bridge_bonus, border_barrier_penalty) =
            calculate_border_metrics(kind, rot, x, &offsets, existing_border_h);

        let adjacency_bonus = calculate_adjacency_bonus(&offsets, x, y, predicted_others);

        let (anti_roof_penalty, hole_fill_bonus) =
            evaluate_roof_and_hole_global(future_board, &sim_future, &offsets, x, y);

        let mut active_vertical_wells: Vec<(usize, usize, usize, usize)> = Vec::new();
        for (wx, wy, wdepth) in future_board.find_lane_vertical_wells(player_id) {
            active_vertical_wells.push((player_id, wx, wy, wdepth));
        }
        for other_signal in &signals.signals {
            if other_signal.lane_id != player_id
                && let Some((wx, wy, wdepth)) = other_signal.vertical_well
            {
                active_vertical_wells.push((other_signal.lane_id, wx, wy, wdepth));
            }
        }

        let (well_cooperation_bonus, well_capping_penalty) = calculate_well_cooperation(
            player_id,
            kind,
            rot,
            x,
            y,
            &offsets,
            &active_vertical_wells,
        );

        let well_creation_penalty = evaluate_well_creation_global(future_board, &sim_future);

        let (signal_cooperation_bonus, non_interference_penalty) =
            calculate_signal_cooperation(player_id, &offsets, x, signals);

        let dist_penalty = calculate_lane_distance_penalty(player_id, x);
        let bottom_priority_bonus = calculate_bottom_priority_bonus(y);

        let hole_clearance_bonus = if future_lines > 0 {
            evaluate_hole_clearance_global(&sim_future, &future_cleared_indices)
        } else {
            0.0
        };

        let lines_val = future_lines as f32 * LINE_WEIGHT + hole_clearance_bonus;

        lines_val
            + (cooperative_lines as f32 * COOPERATIVE_BONUS_WEIGHT)
            + border_bridge_bonus
            + border_barrier_penalty
            + adjacency_bonus
            + anti_roof_penalty
            + hole_fill_bonus
            + signal_cooperation_bonus
            + well_cooperation_bonus
            + well_capping_penalty
            + well_creation_penalty
            + non_interference_penalty
            + dist_penalty
            + (sum_height * HEIGHT_WEIGHT)
            + (max_height * MAX_HEIGHT_WEIGHT)
            + bottom_priority_bonus
            + (holes * HOLES_WEIGHT_GLOBAL)
            + (bumpiness * BUMPINESS_WEIGHT)
    }
}
