pub mod analysis;
pub mod collision;
pub mod compact;
pub mod grid;
pub mod line_clear;
pub mod render;

use crate::config::{LANE_HEIGHT, LANE_WIDTH, TOTAL_GRID_WIDTH};
pub use compact::CompactBoard;
pub use grid::GlobalBoard;

/// 指定した X 座標が境界列（セパレータ列）かどうかを判定
pub fn is_border_column(x: usize) -> bool {
    if x >= TOTAL_GRID_WIDTH {
        return false;
    }
    (x + 1).is_multiple_of(LANE_WIDTH + 1)
}

/// レーン番号 (0..LANE_COUNT) に対応する投入口の X 範囲 (min_x..=max_x)
pub fn lane_x_range(lane_id: usize) -> (usize, usize) {
    let start_x = lane_id * (LANE_WIDTH + 1);
    (start_x, start_x + LANE_WIDTH - 1)
}

impl GlobalBoard {
    /// 指定レーンの投入口スポーン開始座標 (x, y) を取得
    pub fn lane_spawn_origin(lane_id: usize) -> (f32, f32) {
        let (lane_min_x, _) = lane_x_range(lane_id);
        (lane_min_x as f32, (LANE_HEIGHT - 1) as f32)
    }
    /// ビットマスク表現の軽量盤面（Copy可能・ゼロアロケーション）を生成
    pub fn to_compact(&self) -> CompactBoard {
        let mut rows = [0u32; LANE_HEIGHT];
        for (y, row_cells) in self.cells.iter().enumerate() {
            let mut row = 0u32;
            for (x, cell) in row_cells.iter().enumerate() {
                if cell.is_some() {
                    row |= 1u32 << x;
                }
            }
            rows[y] = row;
        }
        CompactBoard { rows }
    }
}
