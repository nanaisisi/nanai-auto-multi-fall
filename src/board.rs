use crate::config::{
    player_color, LANE_HEIGHT, LANE_WIDTH, SPAWN_WALL_MIN_Y, TOTAL_GRID_WIDTH,
};
use crate::tromino::TrominoKind;
use bevy::prelude::*;

/// 指定した X 座標が境界列（セパレータ列）かどうかを判定
pub fn is_border_column(x: usize) -> bool {
    if x >= TOTAL_GRID_WIDTH {
        return false;
    }
    (x + 1) % (LANE_WIDTH + 1) == 0
}

/// レーン番号 (0..LANE_COUNT) に対応する投入口の X 範囲 (min_x..=max_x)
pub fn lane_x_range(lane_id: usize) -> (usize, usize) {
    let start_x = lane_id * (LANE_WIDTH + 1);
    (start_x, start_x + LANE_WIDTH - 1)
}

#[derive(Resource, Debug, Clone)]
pub struct GlobalBoard {
    // [y][x] : y=0 が最下段、y=LANE_HEIGHT-1 が最上段
    pub cells: [[Option<Color>; TOTAL_GRID_WIDTH]; LANE_HEIGHT],
    pub score: u32,
    pub lines_cleared: u32,
    pub game_over: bool,
}

impl Default for GlobalBoard {
    fn default() -> Self {
        Self {
            cells: [[None; TOTAL_GRID_WIDTH]; LANE_HEIGHT],
            score: 0,
            lines_cleared: 0,
            game_over: false,
        }
    }
}

impl GlobalBoard {
    pub fn reset(&mut self) {
        self.cells = [[None; TOTAL_GRID_WIDTH]; LANE_HEIGHT];
        self.game_over = false;
    }

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
            self.game_over = true;
            return false;
        }

        self.score += 10;
        true
    }

    /// 満杯になった行（幅15全セルが埋まった行）を消去し、上方の行を下へシフト
    pub fn clear_full_lines(&mut self) -> usize {
        let mut new_cells = [[None; TOTAL_GRID_WIDTH]; LANE_HEIGHT];
        let mut new_y = 0;
        let mut lines = 0;

        for y in 0..LANE_HEIGHT {
            let is_full = self.cells[y].iter().all(|c| c.is_some());
            if is_full {
                lines += 1;
            } else {
                new_cells[new_y] = self.cells[y];
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

        lines
    }

    /// 各列の高さを取得 (0..LANE_HEIGHT)
    pub fn column_heights(&self) -> [usize; TOTAL_GRID_WIDTH] {
        let mut heights = [0; TOTAL_GRID_WIDTH];
        for x in 0..TOTAL_GRID_WIDTH {
            for y in (0..LANE_HEIGHT).rev() {
                if self.cells[y][x].is_some() {
                    heights[x] = y + 1;
                    break;
                }
            }
        }
        heights
    }

    /// 穴の個数を計算
    pub fn count_holes(&self) -> usize {
        let mut holes = 0;
        for x in 0..TOTAL_GRID_WIDTH {
            let mut block_found = false;
            for y in (0..LANE_HEIGHT).rev() {
                if self.cells[y][x].is_some() {
                    block_found = true;
                } else if block_found {
                    holes += 1;
                }
            }
        }
        holes
    }
}
