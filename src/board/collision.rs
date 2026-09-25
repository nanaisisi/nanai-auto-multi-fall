use super::grid::GlobalBoard;
use super::is_border_column;
use crate::config::{LANE_HEIGHT, SPAWN_WALL_MIN_Y, TOTAL_GRID_WIDTH, player_color};
use crate::tromino::TrominoKind;

impl GlobalBoard {
    /// (x, y) にブロックが存在するか、または外壁・投入口壁かを判定
    pub fn is_occupied(&self, x: i32, y: i32) -> bool {
        // 外壁判定
        if x < 0 || x >= TOTAL_GRID_WIDTH as i32 || y < 0 {
            return true;
        }

        // 上部投入口の境界壁判定 (y >= SPAWN_WALL_MIN_Y かつ 境界列)
        if y >= SPAWN_WALL_MIN_Y as i32 && is_border_column(x as usize) {
            return true;
        }

        if y >= LANE_HEIGHT as i32 {
            // フィールド最上部より上空は空き
            return false;
        }

        self.cells[y as usize][x as usize].is_some()
    }

    /// トミノが (base_x, base_y) に配置可能か（衝突判定）
    pub fn can_place(&self, kind: &TrominoKind, rotation: usize, base_x: i32, base_y: i32) -> bool {
        for (dx, dy) in kind.cell_offsets(rotation) {
            let cx = base_x + dx;
            let cy = base_y + dy;
            if self.is_occupied(cx, cy) {
                return false;
            }
        }
        true
    }

    /// トミノをプレイヤー固有色で盤面に固定する
    pub fn lock_tromino(
        &mut self,
        player_id: usize,
        kind: &TrominoKind,
        rotation: usize,
        base_x: i32,
        base_y: i32,
    ) -> bool {
        let mut top_overflow = false;
        let color = player_color(player_id);

        for (dx, dy) in kind.cell_offsets(rotation) {
            let cx = base_x + dx;
            let cy = base_y + dy;
            if cy >= LANE_HEIGHT as i32 {
                top_overflow = true;
            } else if cx >= 0 && (cx as usize) < TOTAL_GRID_WIDTH && cy >= 0 {
                self.cells[cy as usize][cx as usize] = Some(color);
            }
        }

        if top_overflow {
            self.lane_stuck[player_id % crate::config::LANE_COUNT] = true;
            if self.lane_stuck.iter().all(|&stuck| stuck) {
                self.game_over = true;
            }
            return false;
        }

        self.score += 10;
        self.board_version = self.board_version.wrapping_add(1);
        true
    }
}
