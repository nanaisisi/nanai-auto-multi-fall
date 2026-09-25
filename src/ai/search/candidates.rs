use crate::board::{GlobalBoard, lane_x_range};
use crate::config::TOTAL_GRID_WIDTH;
use crate::game::LanePace;
use crate::tromino::TrominoKind;

/// 各レーンAI自身による自律的なペース判定（遅れや急を要する穴の有無に応じた SoftDrop / Normal 決定）
pub fn determine_autonomous_pace(board: &GlobalBoard, lane_id: usize) -> LanePace {
    let fill_ratio = board.lane_fill_ratio_at_target_line(lane_id);
    let has_urgent_hole = match board.find_lane_deepest_hole(lane_id) {
        Some((_, _, false)) => true, // 露出した穴があり急いで埋めたい
        _ => false,
    };

    if has_urgent_hole || fill_ratio < 0.35 {
        LanePace::SoftDrop
    } else {
        LanePace::Normal
    }
}

/// スポーン時の配置探索候補 (rot, x) のリストを生成
pub fn generate_spawn_candidates(lane_id: usize, kind: &TrominoKind) -> Vec<(usize, i32)> {
    let (lane_min_x, lane_max_x) = lane_x_range(lane_id);
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

    candidates
}

/// 空中再計算時の配置探索候補 (rot, x) のリストを生成（現在位置周辺への拡張付き）
pub fn generate_aerial_candidates(
    lane_id: usize,
    kind: &TrominoKind,
    current_x: i32,
) -> Vec<(usize, i32)> {
    let (lane_min_x, lane_max_x) = lane_x_range(lane_id);
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

    candidates
}
