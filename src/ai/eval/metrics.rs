use crate::board::is_border_column;
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};
use crate::tromino::TrominoKind;

/// 高さ合計、最大高さ、凸凹度 (Bumpiness) を集計
pub fn calculate_height_metrics(heights: &[usize; TOTAL_GRID_WIDTH]) -> (f32, f32, f32) {
    let max_height = *heights.iter().max().unwrap_or(&0) as f32;
    let sum_height: f32 = heights.iter().map(|&h| h as f32).sum();

    let mut bumpiness = 0.0;
    for i in 0..(TOTAL_GRID_WIDTH - 1) {
        bumpiness += (heights[i] as f32 - heights[i + 1] as f32).abs();
    }

    (sum_height, max_height, bumpiness)
}

/// 境界線（セパレータ列）への配置ボーナスおよび境界壁を形成してしまうペナルティを計算
pub fn calculate_border_metrics(
    kind: &TrominoKind,
    rot: usize,
    x: i32,
    offsets: &[(i32, i32)],
    existing_border_col_height: usize,
) -> (f32, f32) {
    let mut border_cell_count = 0;
    for (dx, _) in offsets {
        let cx = (x + dx) as usize;
        if is_border_column(cx) {
            border_cell_count += 1;
        }
    }

    let mut border_bridge_bonus = 0.0;
    let mut border_barrier_penalty = 0.0;

    if border_cell_count > 0 {
        let is_vertical_straight = matches!(kind, TrominoKind::Straight) && (rot % 2 == 1);
        if is_vertical_straight && border_cell_count == 3 {
            if existing_border_col_height > 0 {
                border_barrier_penalty -= 350.0;
            } else {
                border_barrier_penalty -= 220.0;
            }
        } else {
            border_bridge_bonus = border_cell_count as f32 * 18.0;
        }
    }

    (border_bridge_bonus, border_barrier_penalty)
}

/// 低層（床面近く）への配置優先ボーナス
pub fn calculate_bottom_priority_bonus(y: i32) -> f32 {
    ((LANE_HEIGHT as i32 - y).max(0) as f32) * 8.0
}
