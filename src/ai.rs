use crate::board::{is_border_column, lane_x_range, GlobalBoard};
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};
use crate::tromino::TrominoKind;

#[derive(Debug, Clone, Copy)]
pub struct MoveEvaluation {
    pub rotation: usize,
    pub target_x: i32,
    pub landing_y: i32,
    pub score: f32,
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
    /// 特定レーンの投入口からスポーンされたトミノに対する最適配置手を探索
    pub fn find_best_move(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        predicted_others: &[PredictedPlacement],
        air_obstacles: &[(i32, i32)],
    ) -> Option<MoveEvaluation> {
        let mut best_move: Option<MoveEvaluation> = None;
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

        let num_rotations = kind.rotation_count();
        for rot in 0..num_rotations {
            let offsets = kind.cell_offsets(rot);
            let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap();
            let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap();

            // 探索範囲の拡大:
            // 境界列 (x = 3, 7, 11) を跨いで隣接エリアにも十分配置できるよう左右に大きく探索
            let search_min_x = (lane_min_x as i32 - 4).max(-min_dx);
            let search_max_x = (lane_max_x as i32 + 4).min((TOTAL_GRID_WIDTH as i32 - 1) - max_dx);

            for x in search_min_x..=search_max_x {
                if let Some(landing_y) = Self::simulate_drop(
                    board,
                    lane_id,
                    kind,
                    rot,
                    x,
                    &reserved_landing_cells,
                    air_obstacles,
                ) {
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
                        continue;
                    }

                    let eval_score = Self::evaluate_placement(
                        board,
                        &future_board,
                        lane_id,
                        kind,
                        rot,
                        x,
                        landing_y,
                        predicted_others,
                    );

                    if best_move.is_none() || eval_score > best_move.unwrap().score {
                        best_move = Some(MoveEvaluation {
                            rotation: rot,
                            target_x: x,
                            landing_y,
                            score: eval_score,
                        });
                    }
                }
            }
        }

        best_move
    }

    /// 落下シミュレーション
    pub fn simulate_drop(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        rot: usize,
        target_x: i32,
        reserved_landing_cells: &[(i32, i32)],
        air_obstacles: &[(i32, i32)],
    ) -> Option<i32> {
        let (lane_min_x, lane_max_x) = lane_x_range(lane_id);
        let start_y = (LANE_HEIGHT - 1) as i32;

        let spawn_x = lane_min_x as i32;
        if !Self::can_place_check(board, kind, rot, spawn_x, start_y, air_obstacles) {
            return None;
        }

        let offsets = kind.cell_offsets(rot);
        let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap();
        let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap();

        let block_min_x = target_x + min_dx;
        let block_max_x = target_x + max_dx;
        let is_inside_slot =
            block_min_x >= lane_min_x as i32 && block_max_x <= lane_max_x as i32;

        let max_dy = offsets.iter().map(|(_, dy)| *dy).max().unwrap();

        // 投入口外（境界列や隣レーン）へはみ出す配置の場合、
        // 上部セパレータ壁（y >= SPAWN_WALL_MIN_Y）の下端（壁を抜けた地点）から横移動して落下する
        let sim_start_y = if is_inside_slot {
            start_y
        } else {
            (crate::config::SPAWN_WALL_MIN_Y as i32 - 1) - max_dy
        };

        if sim_start_y < 0 {
            return None;
        }

        if !Self::can_place_check(board, kind, rot, target_x, sim_start_y, reserved_landing_cells) {
            return None;
        }

        let mut y = sim_start_y;
        while y > 0 && Self::can_place_check(board, kind, rot, target_x, y - 1, reserved_landing_cells) {
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

    /// 評価関数（境界列の利用インセンティブを含む）
    fn evaluate_placement(
        current_board: &GlobalBoard,
        future_board: &GlobalBoard,
        player_id: usize,
        kind: &TrominoKind,
        rot: usize,
        x: i32,
        y: i32,
        predicted_others: &[PredictedPlacement],
    ) -> f32 {
        let mut sim_future = future_board.clone();
        sim_future.lock_tromino(player_id, kind, rot, x, y);
        let future_lines = sim_future.clear_full_lines();

        let mut sim_alone = current_board.clone();
        sim_alone.lock_tromino(player_id, kind, rot, x, y);
        let alone_lines = sim_alone.clear_full_lines();

        let cooperative_lines = if future_lines > alone_lines {
            future_lines - alone_lines
        } else {
            0
        };

        let heights = sim_future.column_heights();
        let max_height = *heights.iter().max().unwrap_or(&0) as f32;
        let sum_height: f32 = heights.iter().map(|&h| h as f32).sum();
        let holes = sim_future.count_holes() as f32;

        let mut bumpiness = 0.0;
        for i in 0..(TOTAL_GRID_WIDTH - 1) {
            bumpiness += (heights[i] as f32 - heights[i + 1] as f32).abs();
        }

        // 境界列（x = 3, 7, 11）を埋めているかどうかのインセンティブ
        let offsets = kind.cell_offsets(rot);
        let mut border_cell_count = 0;
        for (dx, _) in &offsets {
            let cx = (x + dx) as usize;
            if is_border_column(cx) {
                border_cell_count += 1;
            }
        }
        // 境界列を埋める手、境界を架橋する手にボーナス
        let border_bridge_bonus = border_cell_count as f32 * 18.0;

        // 相手のブロックとの隣接親和性ボーナス
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

        let line_weight = 250.0;
        let cooperative_bonus_weight = 350.0;
        let height_weight = -1.2;
        let max_height_weight = -2.5;
        let holes_weight = -25.0;
        let bumpiness_weight = -1.8;

        (future_lines as f32 * line_weight)
            + (cooperative_lines as f32 * cooperative_bonus_weight)
            + border_bridge_bonus
            + adjacency_bonus
            + (sum_height * height_weight)
            + (max_height * max_height_weight)
            + (holes * holes_weight)
            + (bumpiness * bumpiness_weight)
    }
}
