use super::candidates::{determine_autonomous_pace, generate_aerial_candidates};
use crate::ai::eval::Evaluator;
use crate::ai::pathfinding::PathFinder;
use crate::ai::types::{MoveEvaluation, PredictedPlacement};
use crate::board::GlobalBoard;
use crate::game::LaneSignalBoard;
use crate::tromino::TrominoKind;

/// 落下中の空中現在位置から再計算を行うメソッド（現在の予定に対するコミットメント維持ボーナス対応）
#[allow(clippy::too_many_arguments)]
pub fn find_best_aerial_move(
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
    let autonomous_pace = determine_autonomous_pace(board, lane_id);
    let candidates = generate_aerial_candidates(lane_id, kind, current_x);

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
                eval_score += 120.0;
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
