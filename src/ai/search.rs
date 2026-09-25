use crate::ai::eval::Evaluator;
use crate::ai::pathfinding::PathFinder;
use crate::ai::types::{MoveEvaluation, PredictedPlacement, ScoreBreakdown};
use crate::board::{CompactBoard, GlobalBoard, lane_x_range};
use crate::config::TOTAL_GRID_WIDTH;
use crate::game::{LanePace, LaneSignalBoard};
use crate::tromino::TrominoKind;

pub struct AutoAi;

impl AutoAi {
    /// 特定レーンの投入口からスポーンされたトミノに対する最適配置手を探索（タックイン・スライド対応）
    pub fn find_best_move(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        predicted_others: &[PredictedPlacement],
        air_obstacles: &[(i32, i32)],
        signals: &LaneSignalBoard,
    ) -> Option<MoveEvaluation> {
        let (lane_min_x, lane_max_x) = lane_x_range(lane_id);

        let mut reserved_landing_cells = Vec::new();
        for other in predicted_others {
            for (dx, dy) in other.kind.cell_offsets(other.rotation) {
                reserved_landing_cells.push((other.target_x + dx, other.landing_y + dy));
            }
        }

        let mut future_board = board.clone();
        for other in predicted_others {
            future_board.lock_tromino(
                other.player_id,
                &other.kind,
                other.rotation,
                other.target_x,
                other.landing_y,
            );
        }

        let compact_current = board.to_compact();
        let compact_future = future_board.to_compact();

        // 自レーンAI自身による自律的なペース判定（事前計算）
        let fill_ratio = board.lane_fill_ratio_at_target_line(lane_id);
        let has_urgent_hole = match board.find_lane_deepest_hole(lane_id) {
            Some((_, _, false)) => true, // 露出した穴があり急いで埋めたい
            _ => false,
        };
        let autonomous_pace = if has_urgent_hole || fill_ratio < 0.35 {
            LanePace::SoftDrop
        } else {
            LanePace::Normal
        };

        let mut candidates = Vec::new();
        let num_rotations = kind.rotation_count();
        for rot in 0..num_rotations {
            let offsets = kind.cell_offsets(rot);
            let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap();
            let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap();

            let search_min_x = (lane_min_x as i32 - 3).max(-min_dx);
            let search_max_x = (lane_max_x as i32 + 3).min((TOTAL_GRID_WIDTH as i32 - 1) - max_dx);

            for x in search_min_x..=search_max_x {
                candidates.push((rot, x));
            }
        }

        // 全回転・X座標配置候補のシミュレーション・評価
        candidates
            .into_iter()
            .filter_map(|(rot, x)| {
                let offsets = kind.cell_offsets(rot);
                let (landing_y, waypoints) = PathFinder::simulate_drop_with_tuck_path(
                    &future_board,
                    lane_id,
                    kind,
                    rot,
                    x,
                    &reserved_landing_cells,
                    air_obstacles,
                )?;

                let mut collides = false;
                for (dx, dy) in &offsets {
                    let cx = x + dx;
                    let cy = landing_y + dy;
                    if reserved_landing_cells.contains(&(cx, cy)) {
                        collides = true;
                        break;
                    }
                }
                if collides {
                    return None;
                }

                let (eval_score, breakdown) = Evaluator::evaluate_placement_compact(
                    &compact_current,
                    &compact_future,
                    lane_id,
                    kind,
                    rot,
                    x,
                    landing_y,
                    predicted_others,
                    signals,
                );

                Some(MoveEvaluation {
                    rotation: rot,
                    target_x: x,
                    landing_y,
                    score: eval_score,
                    breakdown,
                    pace: autonomous_pace,
                    waypoints,
                })
            })
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// 落下中の空中現在位置から再計算を行うメソッド（現在の予定に対するコミットメント維持ボーナス対応）
    #[allow(clippy::too_many_arguments)]
    pub fn find_best_move_from_position(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        current_x: i32,
        current_y: i32,
        current_rotation: usize,
        current_target: Option<(i32, usize)>, // (target_x, target_rot)
        predicted_others: &[PredictedPlacement],
        _air_obstacles: &[(i32, i32)],
        signals: &LaneSignalBoard,
    ) -> Option<MoveEvaluation> {
        let (lane_min_x, lane_max_x) = lane_x_range(lane_id);

        let mut reserved_landing_cells = Vec::new();
        for other in predicted_others {
            for (dx, dy) in other.kind.cell_offsets(other.rotation) {
                reserved_landing_cells.push((other.target_x + dx, other.landing_y + dy));
            }
        }

        let mut future_board = board.clone();
        for other in predicted_others {
            future_board.lock_tromino(
                other.player_id,
                &other.kind,
                other.rotation,
                other.target_x,
                other.landing_y,
            );
        }

        let compact_current = board.to_compact();
        let compact_future = future_board.to_compact();

        let fill_ratio = board.lane_fill_ratio_at_target_line(lane_id);
        let has_urgent_hole = matches!(board.find_lane_deepest_hole(lane_id), Some((_, _, false)));
        let autonomous_pace = if has_urgent_hole || fill_ratio < 0.35 {
            LanePace::SoftDrop
        } else {
            LanePace::Normal
        };

        let mut candidates = Vec::new();
        let num_rotations = kind.rotation_count();
        for rot in 0..num_rotations {
            let offsets = kind.cell_offsets(rot);
            let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap();
            let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap();

            let search_min_x = (lane_min_x as i32 - 3).max(-min_dx).min(current_x - 3);
            let search_max_x = (lane_max_x as i32 + 3)
                .min((TOTAL_GRID_WIDTH as i32 - 1) - max_dx)
                .max(current_x + 3);

            for x in search_min_x..=search_max_x {
                candidates.push((rot, x));
            }
        }

        // 空中再計算の探索
        candidates
            .into_iter()
            .filter_map(|(rot, x)| {
                let offsets = kind.cell_offsets(rot);
                let (landing_y, waypoints) = PathFinder::simulate_drop_from_path(
                    &future_board,
                    kind,
                    current_rotation,
                    rot,
                    current_x,
                    current_y,
                    x,
                    &reserved_landing_cells,
                )?;

                let mut collides = false;
                for (dx, dy) in &offsets {
                    let cx = x + dx;
                    let cy = landing_y + dy;
                    if reserved_landing_cells.contains(&(cx, cy)) {
                        collides = true;
                        break;
                    }
                }
                if collides {
                    return None;
                }

                let (mut eval_score, breakdown) = Evaluator::evaluate_placement_compact(
                    &compact_current,
                    &compact_future,
                    lane_id,
                    kind,
                    rot,
                    x,
                    landing_y,
                    predicted_others,
                    signals,
                );

                // 計画維持ボーナス（ヒステリシス）：既に決定済みの目標地点・姿勢を維持する場合にボーナスを付与
                if let Some((cur_tx, cur_rot)) = current_target
                    && x == cur_tx
                    && rot == cur_rot
                {
                    eval_score += 45.0;
                }

                Some(MoveEvaluation {
                    rotation: rot,
                    target_x: x,
                    landing_y,
                    score: eval_score,
                    breakdown,
                    pace: autonomous_pace,
                    waypoints,
                })
            })
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
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
