use super::common::{can_place_check, reconstruct_waypoints};
use crate::board::{GlobalBoard, lane_x_range};
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};
use crate::tromino::TrominoKind;
use std::collections::{HashMap, HashSet, VecDeque};

/// スポーン投入口からの直接垂直落下高速判定
pub fn simulate_direct_drop(
    board: &GlobalBoard,
    lane_id: usize,
    kind: &TrominoKind,
    rot: usize,
    target_x: i32,
    reserved_landing_cells: &[(i32, i32)],
) -> Option<i32> {
    let (lane_min_x, lane_max_x) = lane_x_range(lane_id);
    let start_y = (LANE_HEIGHT - 1) as i32;
    let spawn_x = lane_min_x as i32;

    let offsets = kind.cell_offsets(rot);
    let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap();
    let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap();

    let block_min_x = target_x + min_dx;
    let block_max_x = target_x + max_dx;
    let is_inside_slot = block_min_x >= lane_min_x as i32 && block_max_x <= lane_max_x as i32;

    let max_dy = offsets.iter().map(|(_, dy)| *dy).max().unwrap();

    let sim_start_y = if is_inside_slot {
        start_y
    } else {
        (crate::config::SPAWN_WALL_MIN_Y as i32 - 1) - max_dy
    };

    if sim_start_y < 0 {
        return None;
    }

    let min_step_x = spawn_x.min(target_x);
    let max_step_x = spawn_x.max(target_x);
    for path_x in min_step_x..=max_step_x {
        if !can_place_check(
            board,
            kind,
            rot,
            path_x,
            sim_start_y,
            reserved_landing_cells,
        ) {
            return None;
        }
    }

    let mut y = sim_start_y;
    while y > 0 && can_place_check(board, kind, rot, target_x, y - 1, reserved_landing_cells) {
        y -= 1;
    }

    if !can_place_check(board, kind, rot, target_x, y, reserved_landing_cells) {
        return None;
    }

    Some(y)
}

/// スポーン地点からのタックイン・下くぐりを含むBFS経路探索
pub fn simulate_drop_with_tuck_path(
    board: &GlobalBoard,
    lane_id: usize,
    kind: &TrominoKind,
    rot: usize,
    target_x: i32,
    reserved_landing_cells: &[(i32, i32)],
    air_obstacles: &[(i32, i32)],
) -> Option<(i32, Vec<(i32, i32)>)> {
    let (lane_min_x, lane_max_x) = lane_x_range(lane_id);
    let start_y = (LANE_HEIGHT - 1) as i32;
    let spawn_x = lane_min_x as i32;

    let initial_rotation = match kind {
        TrominoKind::Straight => 1,
        TrominoKind::Corner => 0,
    };
    if !can_place_check(
        board,
        kind,
        initial_rotation,
        spawn_x,
        start_y,
        air_obstacles,
    ) {
        return None;
    }

    let direct_y_opt =
        simulate_direct_drop(board, lane_id, kind, rot, target_x, reserved_landing_cells);

    let offsets = kind.cell_offsets(rot);
    let max_dy = offsets.iter().map(|(_, dy)| *dy).max().unwrap();
    let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap();
    let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap();

    let sim_start_y = (crate::config::SPAWN_WALL_MIN_Y as i32 - 1) - max_dy;
    if sim_start_y < 0 {
        return None;
    }

    for y in (sim_start_y..=start_y).rev() {
        if !can_place_check(board, kind, rot, spawn_x, y, reserved_landing_cells) {
            return None;
        }
    }

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent_map: HashMap<(i32, i32), (i32, i32)> = HashMap::new();

    queue.push_back((spawn_x, sim_start_y));
    visited.insert((spawn_x, sim_start_y));

    let mut lowest_target_y: Option<i32> = None;

    while let Some((cx, cy)) = queue.pop_front() {
        let can_move_down =
            cy > 0 && can_place_check(board, kind, rot, cx, cy - 1, reserved_landing_cells);

        if !can_move_down && cx == target_x {
            match lowest_target_y {
                None => lowest_target_y = Some(cy),
                Some(prev_y) if cy < prev_y => lowest_target_y = Some(cy),
                _ => {}
            }
        }

        if can_move_down {
            let next = (cx, cy - 1);
            if visited.insert(next) {
                parent_map.insert(next, (cx, cy));
                queue.push_back(next);
            }
        }

        let fully_below_spawn_wall = (cy + max_dy) < crate::config::SPAWN_WALL_MIN_Y as i32;
        for dx in [-1, 1] {
            let nx = cx + dx;
            let block_min_x = nx + min_dx;
            let block_max_x = nx + max_dx;
            let is_inside_slot =
                block_min_x >= lane_min_x as i32 && block_max_x <= lane_max_x as i32;

            if (is_inside_slot || fully_below_spawn_wall)
                && nx >= -min_dx
                && nx + max_dx < TOTAL_GRID_WIDTH as i32
                && can_place_check(board, kind, rot, nx, cy, reserved_landing_cells)
            {
                let next = (nx, cy);
                if visited.insert(next) {
                    parent_map.insert(next, (cx, cy));
                    queue.push_back(next);
                }
            }
        }
    }

    let best_y = match (lowest_target_y, direct_y_opt) {
        (Some(bfs_y), Some(dir_y)) => bfs_y.min(dir_y),
        (Some(bfs_y), None) => bfs_y,
        (None, Some(dir_y)) => {
            let block_min_x = target_x + min_dx;
            let block_max_x = target_x + max_dx;
            let is_inside_slot =
                block_min_x >= lane_min_x as i32 && block_max_x <= lane_max_x as i32;

            let wps = if is_inside_slot {
                vec![(target_x, dir_y)]
            } else {
                let wall_clear_y = (crate::config::SPAWN_WALL_MIN_Y as i32 - 1) - max_dy;
                vec![(target_x, wall_clear_y), (target_x, dir_y)]
            };
            return Some((dir_y, wps));
        }
        (None, None) => return None,
    };

    let waypoints = reconstruct_waypoints(&parent_map, target_x, best_y);
    Some((best_y, waypoints))
}
