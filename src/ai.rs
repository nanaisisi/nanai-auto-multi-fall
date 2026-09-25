use crate::board::{CompactBoard, GlobalBoard, is_border_column, lane_x_range};
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};
use crate::game::{HoleStatus, LanePace, LaneSignalBoard};
use crate::tromino::TrominoKind;

use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, Default)]
pub struct ScoreBreakdown {
    pub total: f32,
    pub lines: f32,
    pub coop_lines: f32,
    pub holes_penalty: f32,
    pub height_penalty: f32,
    pub bumpiness_penalty: f32,
    pub anti_roof: f32,
    pub hole_fill: f32,
    pub border: f32,
    pub signal_coop: f32,
    pub well_coop: f32,
}

impl std::fmt::Display for ScoreBreakdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "total:{:+.1} (line:{:+.0}, coop:{:+.0}, holes:{:+.1}, h:{:+.1}, bump:{:+.1}, roof:{:+.0}, fill:{:+.0}, border:{:+.0}, well:{:+.0}, sig:{:+.0})",
            self.total,
            self.lines,
            self.coop_lines,
            self.holes_penalty,
            self.height_penalty,
            self.bumpiness_penalty,
            self.anti_roof,
            self.hole_fill,
            self.border,
            self.well_coop,
            self.signal_coop,
        )
    }
}

#[derive(Debug, Clone)]
pub struct MoveEvaluation {
    pub rotation: usize,
    pub target_x: i32,
    pub landing_y: i32,
    pub score: f32,
    pub breakdown: ScoreBreakdown,
    pub pace: LanePace,
    pub waypoints: Vec<(i32, i32)>,
}

/// 他プレイヤーの着地予測情報
#[derive(Debug, Clone, Copy)]
pub struct PredictedPlacement {
    pub player_id: usize,
    pub kind: TrominoKind,
    pub rotation: usize,
    pub target_x: i32,
    pub landing_y: i32,
}

pub struct AutoAi;

impl AutoAi {
    /// 特定レーンの投入口からスポーンされたトミノに対する最適配置手を探索（タックイン・スライド対応）
    pub fn find_best_move(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        predicted_others: &[PredictedPlacement],
        air_obstacles: &[(i32, i32)],
        signals: &LaneSignalBoard,
    ) -> Option<MoveEvaluation> {
        let (lane_min_x, lane_max_x) = lane_x_range(lane_id);

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

        // 自レーンAI自身による自律的なペース判定（事前計算）
        let fill_ratio = board.lane_fill_ratio_at_target_line(lane_id);
        let has_urgent_hole = match board.find_lane_deepest_hole(lane_id) {
            Some((_, _, false)) => true, // 露出した穴があり急いで埋めたい
            _ => false,
        };
        let autonomous_pace = if has_urgent_hole || fill_ratio < 0.35 {
            LanePace::SoftDrop
        } else {
            LanePace::Normal
        };

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

        // 全回転・X座標配置候補のシミュレーション・評価

        candidates
            .into_iter()
            .filter_map(|(rot, x)| {
                let offsets = kind.cell_offsets(rot);
                let (landing_y, waypoints) = Self::simulate_drop_with_tuck_path(
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

                let (eval_score, breakdown) = Self::evaluate_placement_compact(
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

    /// 落下中の空中現在位置から再計算を行うメソッド（現在の予定に対するコミットメント維持ボーナス対応）
    pub fn find_best_move_from_position(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        current_x: i32,
        current_y: i32,
        current_rotation: usize,
        current_target: Option<(i32, usize)>, // (target_x, target_rot)
        predicted_others: &[PredictedPlacement],
        _air_obstacles: &[(i32, i32)],
        signals: &LaneSignalBoard,
    ) -> Option<MoveEvaluation> {
        let (lane_min_x, lane_max_x) = lane_x_range(lane_id);

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

        let fill_ratio = board.lane_fill_ratio_at_target_line(lane_id);
        let has_urgent_hole = match board.find_lane_deepest_hole(lane_id) {
            Some((_, _, false)) => true,
            _ => false,
        };
        let autonomous_pace = if has_urgent_hole || fill_ratio < 0.35 {
            LanePace::SoftDrop
        } else {
            LanePace::Normal
        };

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

        // 空中再計算の探索

        candidates
            .into_iter()
            .filter_map(|(rot, x)| {
                let offsets = kind.cell_offsets(rot);
                let (landing_y, waypoints) = Self::simulate_drop_from_path(
                    &future_board,
                    kind,
                    current_rotation,
                    rot,
                    current_x,
                    current_y,
                    x,
                    &reserved_landing_cells,
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

                let (mut eval_score, breakdown) = Self::evaluate_placement_compact(
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

                // 計画維持ボーナス（ヒステリシス）：既に決定済みの目標地点・姿勢を維持する場合にボーナスを付与
                // 微小な点数差（他者の微妙な高さ変化など）による目標のブレや玉突き振動を防止する
                if let Some((cur_tx, cur_rot)) = current_target
                    && x == cur_tx
                    && rot == cur_rot
                {
                    eval_score += 45.0; // 計画維持の慣性（大幅な改善または塞がれた場合のみ変更）
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

    /// 空中の現在地点 (start_x, start_y) から目標列 target_x への到達可能性とウェイポイントをBFS探索
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

    fn can_place_check(
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

    /// ビットマスク版CompactBoardを用いたゼロアロケーション・高速評価関数
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
    #[allow(dead_code)]
    fn evaluate_placement(
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

        // 境界列（x = 3, 7, 11...）の評価:
        // 水平方向の架橋（レーン間ブリッジ）は歓迎するが、
        // 縦向きのI字ミノを境界列に重ねて高い障壁（壁）にしてしまう悪手は強力に排除する。
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
                // I字が完全に境界列単独で縦立ちしている状態
                // 境界列にすでにブロックがある上にさらに重ねる場合、あるいは単独で壁を高く立ち上げる場合
                let border_col = x as usize;
                let existing_h = current_board.column_heights()[border_col];
                if existing_h > 0 {
                    // すでにブロックがある境界列にさらにI字を縦積みして壁化 -> 厳禁
                    border_barrier_penalty -= 350.0;
                } else {
                    // 地上からでも境界列に縦I字を立てて障壁化することを強く抑制
                    border_barrier_penalty -= 220.0;
                }
            } else {
                // レーンを跨ぐ横向きミノなどによる健全な境界架橋
                border_bridge_bonus = border_cell_count as f32 * 18.0;
            }
        }

        // 相手のブロックとの隣接親和性
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

        // -------------------------------------------------------------
        // 1. 空白フタ防止ペナルティ & 凹み・穴埋めボーナス
        // -------------------------------------------------------------
        let mut anti_roof_penalty = 0.0;
        let mut hole_fill_bonus = 0.0;

        for (dx, dy) in &offsets {
            let cx = x + dx;
            let cy = y + dy;
            if cx >= 0 && (cx as usize) < TOTAL_GRID_WIDTH {
                // 配置前（future_board）に、このマスの上方にすでに屋根（ブロック）が存在していたか
                let had_roof_above = (cy as usize + 1..LANE_HEIGHT)
                    .any(|above_y| future_board.cells[above_y][cx as usize].is_some());

                if had_roof_above {
                    // すでに屋根がある奥の空洞や凹みに自ら潜り込んで埋めた場合、
                    // フタをしたのではなく「穴埋め／タックイン成功」として高評価ボーナス
                    hole_fill_bonus += 65.0;
                }

                // 自セルより下の段に空洞（未配置マス）があるかをチェック
                let max_check_y = cy.min(LANE_HEIGHT as i32);
                for under_y in 0..max_check_y {
                    if !sim_future.cells[under_y as usize][cx as usize].is_some() {
                        // もしその下の空洞が、幅1マスの縦穴奥かつ高さ2以下で
                        // トミノではどうしようもない（進入不可能な）横空間である場合、
                        // 縦穴解消を優先するためペナルティを大幅に緩和（ほぼ無視）
                        let is_unreachable =
                            future_board.is_unreachable_alcove(cx as usize, under_y as usize);
                        if is_unreachable {
                            anti_roof_penalty -= 5.0;
                            continue;
                        }

                        // もし配置前にもその空洞が存在しており、かつ自セルより上にすでに屋根があった場合は、
                        // 今回の手が新たに塞いだわけではないのでペナルティを大幅に緩和
                        let already_blocked_before = (under_y as usize + 1..LANE_HEIGHT)
                            .any(|above_y| future_board.cells[above_y][cx as usize].is_some());

                        if !already_blocked_before {
                            // 新たに開口部を塞いでフタをしてしまった場合、重いペナルティ
                            anti_roof_penalty -= 80.0;
                        } else {
                            anti_roof_penalty -= 5.0;
                        }
                    }
                }
            }
        }

        // -------------------------------------------------------------
        // 2. 限定シグナルに基づく協調支援と配慮 (Signal-Based Cooperation)
        // -------------------------------------------------------------
        let mut signal_cooperation_bonus = 0.0;
        let mut non_interference_penalty = 0.0;

        // 全レーン（自レーンおよび隣接レーン）の深さ2以上の縦穴情報を集約
        let mut active_vertical_wells: Vec<(usize, usize, usize, usize)> = Vec::new(); // (lane_id, x, bottom_y, depth)
        // 自レーンの縦穴
        for (wx, wy, wdepth) in future_board.find_lane_vertical_wells(player_id) {
            active_vertical_wells.push((player_id, wx, wy, wdepth));
        }
        // 他レーンのシグナルから縦穴情報を取得
        for other_signal in &signals.signals {
            if other_signal.lane_id != player_id
                && let Some((wx, wy, wdepth)) = other_signal.vertical_well
            {
                active_vertical_wells.push((other_signal.lane_id, wx, wy, wdepth));
            }
        }

        // --- 縦穴に対する L字フタ回避 & I字縦穴埋め協力 ---
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
                    // I字トミノ: 縦向き (rot == 1) でこの縦穴の底 (wx, wy) に差し込んで埋める場合
                    if rot % 2 == 1 && x == wx as i32 && y <= wy as i32 {
                        // 縦穴の底にスッポリ収まる配置
                        let bonus = match wdepth {
                            2 => 220.0,
                            3 => 320.0,
                            _ => 260.0,
                        };
                        // 隣接レーンへの救援ならさらに協調ボーナス加算
                        if is_neighbor {
                            well_cooperation_bonus += bonus * 1.5;
                        } else {
                            well_cooperation_bonus += bonus;
                        }
                    }
                }
                TrominoKind::Corner => {
                    // L字トミノ: 縦穴 (wx) の開口部の上 (y >= wy) を塞いでしまう配置を徹底的に避ける
                    // 縦穴の列 wx にセルを置き、かつその下の縦穴マス (wy..wy+wdepth) の一部または全部が空洞のまま残る場合
                    let covers_well_col = offsets.iter().any(|(dx, _)| (x + dx) == wx as i32);
                    if covers_well_col {
                        // 縦穴の底より上で蓋をしてしまうか、あるいは縦穴の開口部付近を塞ぐ場合
                        let lands_above_bottom = offsets
                            .iter()
                            .any(|(dx, dy)| (x + dx) == wx as i32 && (y + dy) > wy as i32);
                        if lands_above_bottom {
                            // 縦穴（深さ2以上）の上を塞ぐ悪手に対して重いペナルティ
                            well_capping_penalty -= 280.0;
                        }
                    }
                }
            }
        }

        // --- 深さ2個以上の縦穴の形成を避ける (Anti Well-Creation Penalty) ---
        // 配置後のシミュレーション盤面（sim_future）において、深さ2以上の縦穴が新たに形成される手を強力に抑止
        let future_wells = sim_future.find_all_vertical_wells();
        let prev_wells = future_board.find_all_vertical_wells();
        let mut well_creation_penalty = 0.0;

        for &(fw_x, _, fw_depth) in &future_wells {
            // この列に配置前にも同等以上の深さの縦穴があったか
            let prev_depth = prev_wells
                .iter()
                .find(|&&(pw_x, _, _)| pw_x == fw_x)
                .map(|&(_, _, pd)| pd)
                .unwrap_or(0);

            if fw_depth >= 2 && fw_depth > prev_depth {
                // 自らの手で深さ2以上の縦穴を新たに作り出したか、または既存の縦穴をさらに深くしてしまった
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

            // 隣接レーンかどうかの判定
            let is_neighbor = (other_signal.lane_id as i32 - player_id as i32).abs() == 1;

            // A. 空白ヘルプ支援
            if let Some(hx) = other_signal.hole_x {
                match other_signal.hole_status {
                    HoleStatus::WaitingForClearance => {
                        // 「まだ上の段が詰まっているので上を塞がないで！」
                        // 当該列 hx の上空に置く手をペナルティ
                        for (dx, _) in &offsets {
                            if (x + dx) == hx as i32 {
                                non_interference_penalty -= 120.0; // 邪魔になるので絶対に置かない
                            }
                        }
                    }
                    HoleStatus::ReadyForFill => {
                        // 「穴が露出して埋められる状態！」
                        // 隣接レーンであれば、支援としてその穴を埋める（または一部を差し込む）手を大きく評価
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

            // B. 人間的な意図宣言（Intent）との不干渉
            if let Some(intent_x) = other_signal.intent_target_x {
                // 相手が「このあたりに置く」と宣言している列付近 (intent_x ± 1)
                for (dx, _) in &offsets {
                    let cx = x + dx;
                    if (cx - intent_x).abs() <= 1 && is_neighbor {
                        // 相手の着地空間を圧迫しないように適度に譲り合う
                        non_interference_penalty -= 25.0;
                    }
                }
            }

            // C. 相手レーンのペース同期（相手が遅延・ソフトドロップで急いでいる場合、相手の領域を侵犯せずサポートに徹する）
            if is_neighbor && other_signal.pace == crate::game::LanePace::SoftDrop {
                // 隣人が急いでいる場合、境界付近を塞ぐような手を控えて譲る
                for (dx, _) in &offsets {
                    let cx = x + dx;
                    if (cx as usize) < TOTAL_GRID_WIDTH && is_border_column(cx as usize) {
                        non_interference_penalty -= 10.0;
                    }
                }
            }
        }

        // -------------------------------------------------------------
        // 3. レーン中心からの距離ペナルティ（基本は自レーン担当）
        // -------------------------------------------------------------
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
