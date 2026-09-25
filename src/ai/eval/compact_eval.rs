use crate::board::CompactBoard;
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};

/// CompactBoard用の空白フタ防止ペナルティ & 凹み・穴埋めボーナス
pub fn evaluate_roof_and_hole_compact(
    future_board: &CompactBoard,
    sim_future: &CompactBoard,
    offsets: &[(i32, i32)],
    x: i32,
    y: i32,
) -> (f32, f32) {
    let mut anti_roof_penalty = 0.0;
    let mut hole_fill_bonus = 0.0;

    for (dx, dy) in offsets {
        let cx = x + dx;
        let cy = y + dy;
        if cx >= 0 && (cx as usize) < TOTAL_GRID_WIDTH {
            let cx_u = cx as usize;
            let bit = 1u32 << cx_u;
            let had_roof_above = (cy as usize + 1..LANE_HEIGHT)
                .any(|above_y| (future_board.rows[above_y] & bit) != 0);

            if had_roof_above {
                hole_fill_bonus += 65.0;
            }

            let max_check_y = cy.min(LANE_HEIGHT as i32);
            for under_y in 0..max_check_y {
                let under_y_u = under_y as usize;
                if (sim_future.rows[under_y_u] & bit) == 0 {
                    let is_unreachable = future_board.is_unreachable_alcove(cx_u, under_y_u);
                    if is_unreachable {
                        anti_roof_penalty -= 5.0;
                        continue;
                    }

                    let already_blocked_before = (under_y_u + 1..LANE_HEIGHT)
                        .any(|above_y| (future_board.rows[above_y] & bit) != 0);

                    if !already_blocked_before {
                        anti_roof_penalty -= 220.0;
                    } else {
                        anti_roof_penalty -= 15.0;
                    }
                }
            }
        }
    }

    (anti_roof_penalty, hole_fill_bonus)
}

/// CompactBoard用の深さ2以上の縦穴新規作成ペナルティ
pub fn evaluate_well_creation_compact(
    future_board: &CompactBoard,
    sim_future: &CompactBoard,
) -> f32 {
    let future_wells = sim_future.find_all_vertical_wells();
    let prev_wells = future_board.find_all_vertical_wells();
    let mut well_creation_penalty = 0.0;

    for &(fw_x, _, fw_depth) in &future_wells {
        let prev_depth = prev_wells
            .iter()
            .find(|&&(pw_x, _, _)| pw_x == fw_x)
            .map(|&(_, _, pd)| pd)
            .unwrap_or(0);

        if fw_depth >= 2 && fw_depth > prev_depth {
            let penalty = match fw_depth {
                2 => 180.0,
                3 => 280.0,
                _ => 360.0,
            };
            well_creation_penalty -= penalty;
        }
    }

    well_creation_penalty
}

/// CompactBoard用のライン消去へ向けた行埋め進行度ボーナス
pub fn evaluate_row_fill_compact(
    sim_future: &CompactBoard,
    offsets: &[(i32, i32)],
    y: i32,
) -> f32 {
    let mut row_fill_bonus = 0.0;
    for (dy, _) in offsets.iter() {
        let row_idx = (y + dy) as usize;
        if row_idx < LANE_HEIGHT {
            let count = sim_future.rows[row_idx].count_ones();
            if count >= 16 {
                let weight = (count as f32 / TOTAL_GRID_WIDTH as f32).powi(2);
                let row_factor = (LANE_HEIGHT - row_idx) as f32;
                row_fill_bonus += weight * row_factor * 12.0;
            }
        }
    }
    row_fill_bonus
}

/// CompactBoard用の穴・空白の上にあるラインの消去ボーナス
pub fn evaluate_hole_clearance_compact(
    sim_future: &CompactBoard,
    future_cleared_indices: &[usize],
) -> f32 {
    let mut hole_clearance_bonus = 0.0;
    for &cleared_y in future_cleared_indices {
        if sim_future.has_hole_below_in_row(cleared_y) {
            hole_clearance_bonus += 120.0;
        }
    }
    hole_clearance_bonus
}
