use crate::board::{GlobalBoard, lane_x_range};
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};
use crate::tromino::TrominoKind;

use std::collections::{HashMap, HashSet, VecDeque};

pub struct PathFinder;

impl PathFinder {
    pub fn can_place_check(
        board: &GlobalBoard,
        kind: &TrominoKind,
        rot: usize,
        base_x: i32,
        base_y: i32,
        obstacles: &[(i32, i32)],
    ) -> bool {
        if !board.can_place(kind, rot, base_x, base_y) {
            return false;
        }
        for (dx, dy) in kind.cell_offsets(rot) {
            let cx = base_x + dx;
            let cy = base_y + dy;
            if obstacles.contains(&(cx, cy)) {
                return false;
            }
        }
        true
    }

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
        let _max_dy = offsets.iter().map(|(_, dy)| *dy).max().unwrap();

        // 垂直落下による直接着地判定
        let mut direct_landing_y = None;
        if start_x == target_x
            && Self::can_place_check(
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
                && Self::can_place_check(
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
            if Self::can_place_check(board, kind, to_rot, target_x, y, reserved_landing_cells) {
                direct_landing_y = Some(y);
            }
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent_map: HashMap<(i32, i32), (i32, i32)> = HashMap::new();

        if !Self::can_place_check(
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
                    && Self::can_place_check(board, kind, to_rot, tx, ty, reserved_landing_cells)
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
                && Self::can_place_check(board, kind, to_rot, cx, cy - 1, reserved_landing_cells);

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
                    && Self::can_place_check(board, kind, to_rot, nx, cy, reserved_landing_cells)
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

        let mut raw_path = Vec::new();
        let mut curr = (target_x, best_y);
        raw_path.push(curr);
        while let Some(&prev) = parent_map.get(&curr) {
            raw_path.push(prev);
            curr = prev;
        }
        raw_path.reverse();

        let mut waypoints = Vec::new();
        if raw_path.len() <= 2 {
            for pt in raw_path {
                if !waypoints.contains(&pt) {
                    waypoints.push(pt);
                }
            }
        } else {
            let mut prev_dir = (raw_path[1].0 - raw_path[0].0, raw_path[1].1 - raw_path[0].1);
            for i in 2..raw_path.len() {
                let cur_dir = (
                    raw_path[i].0 - raw_path[i - 1].0,
                    raw_path[i].1 - raw_path[i - 1].1,
                );
                if cur_dir != prev_dir {
                    waypoints.push(raw_path[i - 1]);
                    prev_dir = cur_dir;
                }
            }
            waypoints.push((target_x, best_y));
        }

        if waypoints.is_empty() {
            waypoints.push((target_x, best_y));
        }

        Some((best_y, waypoints))
    }

    /// 後方互換用: 到達可能最下段Yのみを返すラッパー
    #[allow(dead_code)]
    pub fn simulate_drop_with_tuck(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        rot: usize,
        target_x: i32,
        reserved_landing_cells: &[(i32, i32)],
        air_obstacles: &[(i32, i32)],
    ) -> Option<i32> {
        Self::simulate_drop_with_tuck_path(
            board,
            lane_id,
            kind,
            rot,
            target_x,
            reserved_landing_cells,
            air_obstacles,
        )
        .map(|(y, _)| y)
    }

    /// BFS探索による横移動・下くぐり（タックイン）を含む到達可能最下段Yおよび移動経路（ウェイポイント）の算出
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

        // スポーン口の安全チェック（初期姿勢）
        let initial_rotation = match kind {
            TrominoKind::Straight => 1,
            TrominoKind::Corner => 0,
        };
        if !Self::can_place_check(
            board,
            kind,
            initial_rotation,
            spawn_x,
            start_y,
            air_obstacles,
        ) {
            return None;
        }

        // 垂直落下による直接着地Yの高速取得（存在する場合）
        let direct_y_opt =
            Self::simulate_direct_drop(board, lane_id, kind, rot, target_x, reserved_landing_cells);

        // 垂直落下で届かない場合（オーバーハング下・屋根下・接地横スライドなど）、BFSで経路探索
        let offsets = kind.cell_offsets(rot);
        let max_dy = offsets.iter().map(|(_, dy)| *dy).max().unwrap();
        let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap();
        let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap();

        let sim_start_y = (crate::config::SPAWN_WALL_MIN_Y as i32 - 1) - max_dy;
        if sim_start_y < 0 {
            return None;
        }

        // スポーン口からsim_start_yまで直下移動が可能か
        for y in (sim_start_y..=start_y).rev() {
            if !Self::can_place_check(board, kind, rot, spawn_x, y, reserved_landing_cells) {
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
            let can_move_down = cy > 0
                && Self::can_place_check(board, kind, rot, cx, cy - 1, reserved_landing_cells);

            if !can_move_down && cx == target_x {
                match lowest_target_y {
                    None => lowest_target_y = Some(cy),
                    Some(prev_y) if cy < prev_y => lowest_target_y = Some(cy),
                    _ => {}
                }
            }

            // 下移動
            if can_move_down {
                let next = (cx, cy - 1);
                if visited.insert(next) {
                    parent_map.insert(next, (cx, cy));
                    queue.push_back(next);
                }
            }

            // 左右横移動（スライディング／くぐり）
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
                    && Self::can_place_check(board, kind, rot, nx, cy, reserved_landing_cells)
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

        // 到達点 (target_x, best_y) から parent_map をバックトラックして経路を復元
        let mut raw_path = Vec::new();
        let mut curr = (target_x, best_y);
        raw_path.push(curr);
        while let Some(&prev) = parent_map.get(&curr) {
            raw_path.push(prev);
            curr = prev;
        }
        raw_path.reverse();

        // 経路から折れ曲がり地点（方向が変わる点・最終着地点）のみを抽出してウェイポイント化
        let mut waypoints = Vec::new();
        if raw_path.len() <= 2 {
            for pt in raw_path {
                if !waypoints.contains(&pt) {
                    waypoints.push(pt);
                }
            }
        } else {
            let mut prev_dir = (raw_path[1].0 - raw_path[0].0, raw_path[1].1 - raw_path[0].1);
            for i in 2..raw_path.len() {
                let cur_dir = (
                    raw_path[i].0 - raw_path[i - 1].0,
                    raw_path[i].1 - raw_path[i - 1].1,
                );
                if cur_dir != prev_dir {
                    // 方向転換点
                    waypoints.push(raw_path[i - 1]);
                    prev_dir = cur_dir;
                }
            }
            waypoints.push((target_x, best_y));
        }

        if waypoints.is_empty() {
            waypoints.push((target_x, best_y));
        }

        Some((best_y, waypoints))
    }

    /// 直線移動＋垂直落下の標準シミュレーション
    fn simulate_direct_drop(
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
            if !Self::can_place_check(
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
        while y > 0
            && Self::can_place_check(board, kind, rot, target_x, y - 1, reserved_landing_cells)
        {
            y -= 1;
        }

        if !Self::can_place_check(board, kind, rot, target_x, y, reserved_landing_cells) {
            return None;
        }

        Some(y)
    }
}
