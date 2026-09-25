use super::common::{can_place_check, reconstruct_waypoints};
use crate::board::GlobalBoard;
use crate::config::TOTAL_GRID_WIDTH;
use crate::tromino::TrominoKind;
use std::collections::{HashMap, HashSet, VecDeque};

/// 空中の現在地点 (start_x, start_y) から目標列 target_x への到達可能性とウェイポイントをBFS探索
#[allow(clippy::too_many_arguments)]
pub fn simulate_drop_from_path(
    board: &GlobalBoard,
    kind: &TrominoKind,
    _from_rot: usize,
    to_rot: usize,
    start_x: i32,
    start_y: i32,
    target_x: i32,
    reserved_landing_cells: &[(i32, i32)],
) -> Option<(i32, Vec<(i32, i32)>)> {
    let offsets = kind.cell_offsets(to_rot);
    let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap();
    let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap();

    // 垂直落下による直接着地判定
    let mut direct_landing_y = None;
    if start_x == target_x
        && can_place_check(
            board,
            kind,
            to_rot,
            target_x,
            start_y,
            reserved_landing_cells,
        )
    {
        let mut y = start_y;
        while y > 0
            && can_place_check(
                board,
                kind,
                to_rot,
                target_x,
                y - 1,
                reserved_landing_cells,
            )
        {
            y -= 1;
        }
        if can_place_check(board, kind, to_rot, target_x, y, reserved_landing_cells) {
            direct_landing_y = Some(y);
        }
    }

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent_map: HashMap<(i32, i32), (i32, i32)> = HashMap::new();

    if !can_place_check(
        board,
        kind,
        to_rot,
        start_x,
        start_y,
        reserved_landing_cells,
    ) {
        // 回転直後が衝突する場合はキック位置を起点にする
        let mut kick_found = false;
        for (kdx, kdy) in [(0, 0), (-1, 0), (1, 0), (0, 1), (-1, 1), (1, 1), (0, -1)] {
            let tx = start_x + kdx;
            let ty = start_y + kdy;
            if tx >= -min_dx
                && tx + max_dx < TOTAL_GRID_WIDTH as i32
                && ty >= 0
                && can_place_check(board, kind, to_rot, tx, ty, reserved_landing_cells)
            {
                queue.push_back((tx, ty));
                visited.insert((tx, ty));
                kick_found = true;
                break;
            }
        }
        if !kick_found {
            return None;
        }
    } else {
        queue.push_back((start_x, start_y));
        visited.insert((start_x, start_y));
    }

    let mut lowest_target_y: Option<i32> = None;

    while let Some((cx, cy)) = queue.pop_front() {
        let can_move_down = cy > 0
            && can_place_check(board, kind, to_rot, cx, cy - 1, reserved_landing_cells);

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

        for dx in [-1, 1] {
            let nx = cx + dx;
            if nx >= -min_dx
                && nx + max_dx < TOTAL_GRID_WIDTH as i32
                && can_place_check(board, kind, to_rot, nx, cy, reserved_landing_cells)
            {
                let next = (nx, cy);
                if visited.insert(next) {
                    parent_map.insert(next, (cx, cy));
                    queue.push_back(next);
                }
            }
        }
    }

    let best_y = match (lowest_target_y, direct_landing_y) {
        (Some(bfs_y), Some(dir_y)) => bfs_y.min(dir_y),
        (Some(bfs_y), None) => bfs_y,
        (None, Some(dir_y)) => return Some((dir_y, vec![(target_x, dir_y)])),
        (None, None) => return None,
    };

    let waypoints = reconstruct_waypoints(&parent_map, target_x, best_y);
    Some((best_y, waypoints))
}
