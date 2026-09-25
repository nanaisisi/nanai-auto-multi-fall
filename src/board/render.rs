use super::grid::GlobalBoard;
use super::is_border_column;
use crate::config::{LANE_HEIGHT, SPAWN_WALL_MIN_Y, TOTAL_GRID_WIDTH};

impl GlobalBoard {
    /// 盤面の高さを棒グラフ・数値で表現したサマリー文字列を生成
    #[allow(dead_code)]
    pub fn render_height_profile(&self) -> String {
        let heights = self.column_heights();
        let mut s = String::new();
        s.push_str("Col Heights: [");
        for (i, &h) in heights.iter().enumerate() {
            if i > 0 && is_border_column(i - 1) {
                s.push('|');
            }
            s.push_str(&format!("{:>2} ", h));
        }
        s.push(']');
        s
    }

    /// 盤面全体のASCIIスナップショット（境界壁・ブロック・穴・空き）を生成
    pub fn render_ascii(&self) -> String {
        let mut out = String::new();
        let holes = self.count_holes();
        let heights = self.column_heights();
        let max_h = heights.iter().max().copied().unwrap_or(0);
        let avg_h: f32 = heights.iter().sum::<usize>() as f32 / TOTAL_GRID_WIDTH as f32;

        out.push_str(&format!(
            "--- BOARD SNAPSHOT (Lines: {}, Score: {}, Holes: {}, MaxH: {}, AvgH: {:.1}) ---\n",
            self.lines_cleared, self.score, holes, max_h, avg_h
        ));

        out.push_str("    +");
        for x in 0..TOTAL_GRID_WIDTH {
            if is_border_column(x) {
                out.push('|');
            } else {
                out.push('-');
            }
        }
        out.push_str("+\n");

        for y in (0..LANE_HEIGHT).rev() {
            out.push_str(&format!("{:>2} |", y));
            for x in 0..TOTAL_GRID_WIDTH {
                if is_border_column(x) {
                    if y >= SPAWN_WALL_MIN_Y {
                        out.push('|');
                    } else if self.cells[y][x].is_some() {
                        out.push('#');
                    } else {
                        out.push(':');
                    }
                } else if self.cells[y][x].is_some() {
                    out.push('#');
                } else {
                    let is_hole = (y + 1..LANE_HEIGHT).any(|hy| self.cells[hy][x].is_some());
                    if is_hole {
                        out.push('.');
                    } else {
                        out.push(' ');
                    }
                }
            }
            out.push_str("|\n");
        }

        out.push_str("    +");
        for x in 0..TOTAL_GRID_WIDTH {
            if is_border_column(x) {
                out.push('+');
            } else {
                out.push('-');
            }
        }
        out.push_str("+\n");

        out.push_str("     ");
        for lane in 0..crate::config::LANE_COUNT {
            out.push_str(&format!(" L{} ", lane + 1));
            if lane < crate::config::LANE_COUNT - 1 {
                out.push(' ');
            }
        }
        out.push('\n');

        out
    }
}
