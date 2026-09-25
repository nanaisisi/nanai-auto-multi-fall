use crate::ai::types::PredictedPlacement;
use crate::board::{is_border_column, lane_x_range};
use crate::config::TOTAL_GRID_WIDTH;
use crate::game::{HoleStatus, LanePace, LaneSignalBoard};
use crate::tromino::TrominoKind;

/// 他プレイヤーとの隣接ボーナスを計算
pub fn calculate_adjacency_bonus(
    offsets: &[(i32, i32)],
    x: i32,
    y: i32,
    predicted_others: &[PredictedPlacement],
) -> f32 {
    let mut adjacency_bonus = 0.0;
    for other in predicted_others {
        let other_offsets = other.kind.cell_offsets(other.rotation);
        for (dx1, dy1) in offsets {
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
    adjacency_bonus
}

/// レーン中心からの距離ペナルティ（自分の持ち場を優先し、遠くへ侵入しない）
pub fn calculate_lane_distance_penalty(player_id: usize, x: i32) -> f32 {
    let (lane_min_x, lane_max_x) = lane_x_range(player_id);
    let lane_center_x = (lane_min_x + lane_max_x) as f32 / 2.0;
    let dist_from_lane = (x as f32 - lane_center_x).abs();
    dist_from_lane * -6.0
}

/// 限定シグナルに基づく協調支援と配慮（不干渉ペナルティ・シグナル協力ボーナス）
pub fn calculate_signal_cooperation(
    player_id: usize,
    offsets: &[(i32, i32)],
    x: i32,
    signals: &LaneSignalBoard,
) -> (f32, f32) {
    let mut signal_cooperation_bonus = 0.0;
    let mut non_interference_penalty = 0.0;

    for other_signal in &signals.signals {
        if other_signal.lane_id == player_id {
            continue;
        }

        let is_neighbor = (other_signal.lane_id as i32 - player_id as i32).abs() == 1;

        if let Some(hx) = other_signal.hole_x {
            match other_signal.hole_status {
                HoleStatus::WaitingForClearance => {
                    for (dx, _) in offsets {
                        if (x + dx) == hx as i32 {
                            non_interference_penalty -= 120.0;
                        }
                    }
                }
                HoleStatus::ReadyForFill => {
                    if is_neighbor {
                        for (dx, _) in offsets {
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
            for (dx, _) in offsets {
                let cx = x + dx;
                if (cx - intent_x).abs() <= 1 && is_neighbor {
                    non_interference_penalty -= 25.0;
                }
            }
        }

        if is_neighbor && other_signal.pace == LanePace::SoftDrop {
            for (dx, _) in offsets {
                let cx = x + dx;
                if (cx as usize) < TOTAL_GRID_WIDTH && is_border_column(cx as usize) {
                    non_interference_penalty -= 30.0;
                }
            }
        }
    }

    (signal_cooperation_bonus, non_interference_penalty)
}

/// 縦穴（Vertical Wells）の活用および蓋をしてしまうペナルティを計算
pub fn calculate_well_cooperation(
    player_id: usize,
    kind: &TrominoKind,
    rot: usize,
    x: i32,
    y: i32,
    offsets: &[(i32, i32)],
    active_vertical_wells: &[(usize, usize, usize, usize)],
) -> (f32, f32) {
    let mut well_cooperation_bonus = 0.0;
    let mut well_capping_penalty = 0.0;

    for &(well_lane_id, wx, wy, wdepth) in active_vertical_wells {
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

    (well_cooperation_bonus, well_capping_penalty)
}
