use super::grid::GlobalBoard;
use super::lane_x_range;
use crate::config::{LANE_HEIGHT, SPAWN_WALL_MIN_Y, TOTAL_GRID_WIDTH};

impl GlobalBoard {
    /// 満杯になった行（幅15全セルが埋まった行）を消去し、上方の行を下へシフト
    pub fn clear_full_lines(&mut self) -> usize {
        let (lines, _) = self.clear_full_lines_with_indices();
        lines
    }

    /// 行消去処理（消去行数および消去された行の元のy座標リストを返す）
    pub fn clear_full_lines_with_indices(&mut self) -> (usize, Vec<usize>) {
        let mut new_cells = [[None; TOTAL_GRID_WIDTH]; LANE_HEIGHT];
        let mut new_y = 0;
        let mut lines = 0;
        let mut cleared_indices = Vec::new();

        for (orig_y, row) in self.cells.iter().enumerate() {
            let is_full = row.iter().all(|c| c.is_some());
            if is_full {
                lines += 1;
                cleared_indices.push(orig_y);
            } else {
                new_cells[new_y] = *row;
                new_y += 1;
            }
        }

        self.cells = new_cells;
        self.lines_cleared += lines as u32;
        self.score += match lines {
            1 => 300,
            2 => 800,
            3 => 1800,
            _ => lines as u32 * 500,
        };

        if lines > 0 {
            self.board_version = self.board_version.wrapping_add(1);
            // 下のブロックが消去されたため、各レーンの投入口詰まりを再評価
            for lane_id in 0..crate::config::LANE_COUNT {
                let (min_x, max_x) = lane_x_range(lane_id);
                // 投入口の上部（SPAWN_WALL_MIN_Y以上）にブロックが残っていなければ積み解除
                let still_blocked = (SPAWN_WALL_MIN_Y..LANE_HEIGHT)
                    .any(|y| (min_x..=max_x).any(|x| self.cells[y][x].is_some()));
                if !still_blocked {
                    self.lane_stuck[lane_id] = false;
                }
            }
            // まだ全員が詰まっていない場合はゲームオーバー状態も復帰
            if !self.lane_stuck.iter().all(|&stuck| stuck) {
                self.game_over = false;
            }
        }

        (lines, cleared_indices)
    }
}
