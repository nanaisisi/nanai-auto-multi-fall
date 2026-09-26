use super::candidates::{determine_autonomous_pace, generate_spawn_candidates};
use crate::ai::eval::Evaluator;
use crate::ai::pathfinding::PathFinder;
use crate::ai::types::{MoveEvaluation, PredictedPlacement};
use crate::board::GlobalBoard;
use crate::game::LaneSignalBoard;
use crate::tromino::TrominoKind;

/// 特定レーンの投入口からスポーンされたトミノに対する最適配置手を探索（タックイン・スライド対応）
pub fn find_best_spawn_move(
    board: &GlobalBoard,
    lane_id: usize,
    kind: &TrominoKind,
    predicted_others: &[PredictedPlacement],
    air_obstacles: &[(i32, i32)],
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
    let candidates = generate_spawn_candidates(lane_id, kind);

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

            if x == 0 {
                println!(
                    "DEBUG cand: rot={}, landing_y={}, score={}, lines={}, holes={}",
                    rot, landing_y, eval_score, breakdown.lines, breakdown.holes_penalty
                );
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
