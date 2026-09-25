use crate::board::GlobalBoard;
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};

/// GlobalBoard用の空白フタ防止ペナルティ & 凹み・穴埋めボーナス
pub fn evaluate_roof_and_hole_global(
    future_board: &GlobalBoard,
    sim_future: &GlobalBoard,
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
            let had_roof_above = (cy as usize + 1..LANE_HEIGHT)
                .any(|above_y| future_board.cells[above_y][cx as usize].is_some());

            if had_roof_above {
                hole_fill_bonus += 65.0;
            }

            let max_check_y = cy.min(LANE_HEIGHT as i32);
            for under_y in 0..max_check_y {
                if sim_future.cells[under_y as usize][cx as usize].is_none() {
                    let is_unreachable =
                        future_board.is_unreachable_alcove(cx as usize, under_y as usize);
                    if is_unreachable {
                        anti_roof_penalty -= 5.0;
                        continue;
                    }

                    let already_blocked_before = (under_y as usize + 1..LANE_HEIGHT)
                        .any(|above_y| future_board.cells[above_y][cx as usize].is_some());

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

/// GlobalBoard用の深さ2以上の縦穴新規作成ペナルティ
pub fn evaluate_well_creation_global(future_board: &GlobalBoard, sim_future: &GlobalBoard) -> f32 {
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

/// GlobalBoard用の穴・空白の上にあるラインの消去ボーナス
pub fn evaluate_hole_clearance_global(
    sim_future: &GlobalBoard,
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
