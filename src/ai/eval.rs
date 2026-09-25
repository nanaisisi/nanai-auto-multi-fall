use crate::ai::types::{PredictedPlacement, ScoreBreakdown};
use crate::board::{CompactBoard, GlobalBoard, is_border_column, lane_x_range};
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};
use crate::game::{HoleStatus, LaneSignalBoard};
use crate::tromino::TrominoKind;

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
        let future_lines = sim_future.clear_full_lines();

        let mut sim_alone = *current_board;
        sim_alone.lock_tromino(kind, rot, x, y);
        let alone_lines = sim_alone.clear_full_lines();

        let cooperative_lines = future_lines.saturating_sub(alone_lines);

        let heights = sim_future.column_heights();
        let max_height = *heights.iter().max().unwrap_or(&0) as f32;
        let sum_height: f32 = heights.iter().map(|&h| h as f32).sum();
        let holes = sim_future.count_holes() as f32;

        let mut bumpiness = 0.0;
        for i in 0..(TOTAL_GRID_WIDTH - 1) {
            bumpiness += (heights[i] as f32 - heights[i + 1] as f32).abs();
        }

        let offsets = kind.cell_offsets(rot);
        let mut border_cell_count = 0;
        for (dx, _) in &offsets {
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
                let border_col = x as usize;
                let existing_h = current_board.column_heights()[border_col];
                if existing_h > 0 {
                    border_barrier_penalty -= 350.0;
                } else {
                    border_barrier_penalty -= 220.0;
                }
            } else {
                border_bridge_bonus = border_cell_count as f32 * 18.0;
            }
        }

        let mut adjacency_bonus = 0.0;
        for other in predicted_others {
            let other_offsets = other.kind.cell_offsets(other.rotation);
            for (dx1, dy1) in &offsets {
                let my_c = (x + dx1, y + dy1);
                for (dx2, dy2) in &other_offsets {
                    let ot_c = (other.target_x + dx2, other.landing_y + dy2);
                    let dist = (my_c.0 - ot_c.0).abs() + (my_c.1 - ot_c.1).abs();
                    if dist == 1 {
                        adjacency_bonus += 6.0;
                    }
                }
            }
        }

        // 1. 空白フタ防止ペナルティ & 凹み・穴埋めボーナス
        let mut anti_roof_penalty = 0.0;
        let mut hole_fill_bonus = 0.0;

        for (dx, dy) in &offsets {
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
                            anti_roof_penalty -= 80.0;
                        } else {
                            anti_roof_penalty -= 5.0;
                        }
                    }
                }
            }
        }

        // 2. 限定シグナルに基づく協調支援と配慮
        let mut signal_cooperation_bonus = 0.0;
        let mut non_interference_penalty = 0.0;

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

        let mut well_cooperation_bonus = 0.0;
        let mut well_capping_penalty = 0.0;

        for &(well_lane_id, wx, wy, wdepth) in &active_vertical_wells {
            let is_own_or_neighbor = (well_lane_id as i32 - player_id as i32).abs() <= 1;
            if !is_own_or_neighbor {
                continue;
            }

            let is_neighbor = (well_lane_id as i32 - player_id as i32).abs() == 1;

            match kind {
                TrominoKind::Straight => {
                    if rot % 2 == 1 && x == wx as i32 && y <= wy as i32 {
                        let bonus = match wdepth {
                            2 => 220.0,
                            3 => 320.0,
                            _ => 260.0,
                        };
                        if is_neighbor {
                            well_cooperation_bonus += bonus * 1.5;
                        } else {
                            well_cooperation_bonus += bonus;
                        }
                    }
                }
                TrominoKind::Corner => {
                    let covers_well_col = offsets.iter().any(|(dx, _)| (x + dx) == wx as i32);
                    if covers_well_col {
                        let lands_above_bottom = offsets
                            .iter()
                            .any(|(dx, dy)| (x + dx) == wx as i32 && (y + dy) > wy as i32);
                        if lands_above_bottom {
                            well_capping_penalty -= 280.0;
                        }
                    }
                }
            }
        }

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

        for other_signal in &signals.signals {
            if other_signal.lane_id == player_id {
                continue;
            }

            let is_neighbor = (other_signal.lane_id as i32 - player_id as i32).abs() == 1;

            if let Some(hx) = other_signal.hole_x {
                match other_signal.hole_status {
                    HoleStatus::WaitingForClearance => {
                        for (dx, _) in &offsets {
                            if (x + dx) == hx as i32 {
                                non_interference_penalty -= 120.0;
                            }
                        }
                    }
                    HoleStatus::ReadyForFill => {
                        if is_neighbor {
                            for (dx, _) in &offsets {
                                if (x + dx) == hx as i32 {
                                    signal_cooperation_bonus += 140.0;
                                }
                            }
                        }
                    }
                    HoleStatus::None => {}
                }
            }

            if let Some(intent_x) = other_signal.intent_target_x {
                for (dx, _) in &offsets {
                    let cx = x + dx;
                    if (cx - intent_x).abs() <= 1 && is_neighbor {
                        non_interference_penalty -= 25.0;
                    }
                }
            }

            if is_neighbor && other_signal.pace == crate::game::LanePace::SoftDrop {
                for (dx, _) in &offsets {
                    let cx = x + dx;
                    if (cx as usize) < TOTAL_GRID_WIDTH && is_border_column(cx as usize) {
                        non_interference_penalty -= 10.0;
                    }
                }
            }
        }

        // 3. レーン中心からの距離ペナルティ
        let (lane_min_x, lane_max_x) = lane_x_range(player_id);
        let lane_center_x = (lane_min_x + lane_max_x) as f32 / 2.0;
        let dist_from_lane = (x as f32 - lane_center_x).abs();
        let dist_penalty = dist_from_lane * -3.5;

        let line_weight = 250.0;
        let cooperative_bonus_weight = 350.0;
        let height_weight = -1.2;
        let max_height_weight = -2.5;
        let holes_weight = -30.0;
        let bumpiness_weight = -1.8;

        let lines_val = future_lines as f32 * line_weight;
        let coop_lines_val = cooperative_lines as f32 * cooperative_bonus_weight;
        let holes_val = holes * holes_weight;
        let height_val = (sum_height * height_weight) + (max_height * max_height_weight);
        let bumpiness_val = bumpiness * bumpiness_weight;
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
        let future_lines = sim_future.clear_full_lines();

        let mut sim_alone = current_board.clone();
        sim_alone.lock_tromino(player_id, kind, rot, x, y);
        let alone_lines = sim_alone.clear_full_lines();

        let cooperative_lines = future_lines.saturating_sub(alone_lines);

        let heights = sim_future.column_heights();
        let max_height = *heights.iter().max().unwrap_or(&0) as f32;
        let sum_height: f32 = heights.iter().map(|&h| h as f32).sum();
        let holes = sim_future.count_holes() as f32;

        let mut bumpiness = 0.0;
        for i in 0..(TOTAL_GRID_WIDTH - 1) {
            bumpiness += (heights[i] as f32 - heights[i + 1] as f32).abs();
        }

        let offsets = kind.cell_offsets(rot);
        let mut border_cell_count = 0;
        for (dx, _) in &offsets {
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
                let border_col = x as usize;
                let existing_h = current_board.column_heights()[border_col];
                if existing_h > 0 {
                    border_barrier_penalty -= 350.0;
                } else {
                    border_barrier_penalty -= 220.0;
                }
            } else {
                border_bridge_bonus = border_cell_count as f32 * 18.0;
            }
        }

        let mut adjacency_bonus = 0.0;
        for other in predicted_others {
            let other_offsets = other.kind.cell_offsets(other.rotation);
            for (dx1, dy1) in &offsets {
                let my_c = (x + dx1, y + dy1);
                for (dx2, dy2) in &other_offsets {
                    let ot_c = (other.target_x + dx2, other.landing_y + dy2);
                    let dist = (my_c.0 - ot_c.0).abs() + (my_c.1 - ot_c.1).abs();
                    if dist == 1 {
                        adjacency_bonus += 6.0;
                    }
                }
            }
        }

        let mut anti_roof_penalty = 0.0;
        let mut hole_fill_bonus = 0.0;

        for (dx, dy) in &offsets {
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
                            anti_roof_penalty -= 80.0;
                        } else {
                            anti_roof_penalty -= 5.0;
                        }
                    }
                }
            }
        }

        let mut signal_cooperation_bonus = 0.0;
        let mut non_interference_penalty = 0.0;

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

        let mut well_cooperation_bonus = 0.0;
        let mut well_capping_penalty = 0.0;

        for &(well_lane_id, wx, wy, wdepth) in &active_vertical_wells {
            let is_own_or_neighbor = (well_lane_id as i32 - player_id as i32).abs() <= 1;
            if !is_own_or_neighbor {
                continue;
            }

            let is_neighbor = (well_lane_id as i32 - player_id as i32).abs() == 1;

            match kind {
                TrominoKind::Straight => {
                    if rot % 2 == 1 && x == wx as i32 && y <= wy as i32 {
                        let bonus = match wdepth {
                            2 => 220.0,
                            3 => 320.0,
                            _ => 260.0,
                        };
                        if is_neighbor {
                            well_cooperation_bonus += bonus * 1.5;
                        } else {
                            well_cooperation_bonus += bonus;
                        }
                    }
                }
                TrominoKind::Corner => {
                    let covers_well_col = offsets.iter().any(|(dx, _)| (x + dx) == wx as i32);
                    if covers_well_col {
                        let lands_above_bottom = offsets
                            .iter()
                            .any(|(dx, dy)| (x + dx) == wx as i32 && (y + dy) > wy as i32);
                        if lands_above_bottom {
                            well_capping_penalty -= 280.0;
                        }
                    }
                }
            }
        }

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

        for other_signal in &signals.signals {
            if other_signal.lane_id == player_id {
                continue;
            }

            let is_neighbor = (other_signal.lane_id as i32 - player_id as i32).abs() == 1;

            if let Some(hx) = other_signal.hole_x {
                match other_signal.hole_status {
                    HoleStatus::WaitingForClearance => {
                        for (dx, _) in &offsets {
                            if (x + dx) == hx as i32 {
                                non_interference_penalty -= 120.0;
                            }
                        }
                    }
                    HoleStatus::ReadyForFill => {
                        if is_neighbor {
                            for (dx, _) in &offsets {
                                if (x + dx) == hx as i32 {
                                    signal_cooperation_bonus += 140.0;
                                }
                            }
                        }
                    }
                    HoleStatus::None => {}
                }
            }

            if let Some(intent_x) = other_signal.intent_target_x {
                for (dx, _) in &offsets {
                    let cx = x + dx;
                    if (cx - intent_x).abs() <= 1 && is_neighbor {
                        non_interference_penalty -= 25.0;
                    }
                }
            }

            if is_neighbor && other_signal.pace == crate::game::LanePace::SoftDrop {
                for (dx, _) in &offsets {
                    let cx = x + dx;
                    if (cx as usize) < TOTAL_GRID_WIDTH && is_border_column(cx as usize) {
                        non_interference_penalty -= 10.0;
                    }
                }
            }
        }

        let (lane_min_x, lane_max_x) = lane_x_range(player_id);
        let lane_center_x = (lane_min_x + lane_max_x) as f32 / 2.0;
        let dist_from_lane = (x as f32 - lane_center_x).abs();
        let dist_penalty = dist_from_lane * -3.5;

        let line_weight = 250.0;
        let cooperative_bonus_weight = 350.0;
        let height_weight = -1.2;
        let max_height_weight = -2.5;
        let holes_weight = -30.0;
        let bumpiness_weight = -1.8;

        (future_lines as f32 * line_weight)
            + (cooperative_lines as f32 * cooperative_bonus_weight)
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
            + (sum_height * height_weight)
            + (max_height * max_height_weight)
            + (holes * holes_weight)
            + (bumpiness * bumpiness_weight)
    }
}
