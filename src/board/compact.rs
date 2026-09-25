use crate::board::{is_border_column, lane_x_range};
use crate::config::{LANE_HEIGHT, SPAWN_WALL_MIN_Y, TOTAL_GRID_WIDTH};
use crate::tromino::TrominoKind;

/// 31ビット幅のu32行配列による超高速・スタック完結・Copy可能なビットボード
/// AI探索時の一時シミュレーション（ライン消去・穴判定・高さ計算）をゼロアロケーションで行う
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactBoard {
    pub rows: [u32; LANE_HEIGHT],
}

impl CompactBoard {
    pub const FULL_ROW_MASK: u32 = (1u32 << TOTAL_GRID_WIDTH) - 1;

    #[allow(dead_code)]
    #[inline(always)]
    pub fn is_occupied(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= TOTAL_GRID_WIDTH as i32 || y < 0 {
            return true;
        }
        if y >= SPAWN_WALL_MIN_Y as i32 && is_border_column(x as usize) {
            return true;
        }
        if y >= LANE_HEIGHT as i32 {
            return false;
        }
        (self.rows[y as usize] & (1u32 << (x as usize))) != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
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

    #[inline(always)]
    pub fn lock_tromino(
        &mut self,
        kind: &TrominoKind,
        rotation: usize,
        base_x: i32,
        base_y: i32,
    ) -> bool {
        let mut top_overflow = false;
        for (dx, dy) in kind.cell_offsets(rotation) {
            let cx = base_x + dx;
            let cy = base_y + dy;
            if cy >= LANE_HEIGHT as i32 {
                top_overflow = true;
            } else if cx >= 0 && (cx as usize) < TOTAL_GRID_WIDTH && cy >= 0 {
                self.rows[cy as usize] |= 1u32 << (cx as usize);
            }
        }
        !top_overflow
    }

    /// 行消去シミュレーション（フルラインをビットマスク判定し、シフト）
    #[inline(always)]
    pub fn clear_full_lines(&mut self) -> usize {
        let (lines, _) = self.clear_full_lines_with_indices();
        lines
    }

    /// 行消去シミュレーション（消去行数および消去された行の元のy座標リストを返す）
    #[inline(always)]
    pub fn clear_full_lines_with_indices(&mut self) -> (usize, Vec<usize>) {
        let mut new_rows = [0u32; LANE_HEIGHT];
        let mut new_y = 0;
        let mut lines = 0;
        let mut cleared_indices = Vec::new();

        for (orig_y, row) in self.rows.iter().enumerate() {
            if (*row & Self::FULL_ROW_MASK) == Self::FULL_ROW_MASK {
                lines += 1;
                cleared_indices.push(orig_y);
            } else {
                new_rows[new_y] = *row;
                new_y += 1;
            }
        }

        self.rows = new_rows;
        (lines, cleared_indices)
    }

    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    pub fn column_heights(&self) -> [usize; TOTAL_GRID_WIDTH] {
        let mut heights = [0; TOTAL_GRID_WIDTH];
        for x in 0..TOTAL_GRID_WIDTH {
            let bit = 1u32 << x;
            for y in (0..LANE_HEIGHT).rev() {
                if (self.rows[y] & bit) != 0 {
                    heights[x] = y + 1;
                    break;
                }
            }
        }
        heights
    }

    #[inline(always)]
    pub fn count_holes(&self) -> usize {
        let mut holes = 0;
        for x in 0..TOTAL_GRID_WIDTH {
            let bit = 1u32 << x;
            let mut block_found = false;
            for y in (0..LANE_HEIGHT).rev() {
                if (self.rows[y] & bit) != 0 {
                    block_found = true;
                } else if block_found {
                    holes += 1;
                }
            }
        }
        holes
    }

    /// 盤面全体で最も高い位置にある穴（最上位の空白）のy座標を返す
    #[allow(dead_code)]
    #[inline(always)]
    pub fn highest_hole_y(&self) -> Option<usize> {
        let mut max_y = None;
        for x in 0..TOTAL_GRID_WIDTH {
            let bit = 1u32 << x;
            let mut roof_found = false;
            for y in (0..LANE_HEIGHT).rev() {
                if (self.rows[y] & bit) != 0 {
                    roof_found = true;
                } else if roof_found {
                    max_y = Some(max_y.map_or(y, |prev: usize| prev.max(y)));
                    break;
                }
            }
        }
        max_y
    }

    /// 指定した行 y の下に塞がれている空白（穴）が存在するかどうか
    /// （＝この行を消すことで、直下にある穴の天井が削られて救出・露出に貢献するか）
    #[inline(always)]
    pub fn has_hole_below_in_row(&self, y: usize) -> bool {
        if y == 0 {
            return false;
        }
        // y より下の行のいずれかに 0（空白）ビットがある列を探す
        // かつ、その空白の上にブロックが存在している（＝穴である）
        for x in 0..TOTAL_GRID_WIDTH {
            let bit = 1u32 << x;
            // この列で y にブロックがあるか、もしくは y 以上のどこかに屋根があるか
            let has_roof = (y..LANE_HEIGHT).any(|ry| (self.rows[ry] & bit) != 0);
            if has_roof {
                // y より下に空白マスがあるか
                for uy in 0..y {
                    if (self.rows[uy] & bit) == 0 {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 盤面全体で最も低い位置にある穴（最下層の空白）のy座標を返す
    #[allow(dead_code)]
    #[inline(always)]
    pub fn lowest_hole_y(&self) -> Option<usize> {
        let mut min_y = None;
        for x in 0..TOTAL_GRID_WIDTH {
            let bit = 1u32 << x;
            let mut roof_found = false;
            for y in (0..LANE_HEIGHT).rev() {
                if (self.rows[y] & bit) != 0 {
                    roof_found = true;
                } else if roof_found {
                    min_y = Some(min_y.map_or(y, |prev: usize| prev.min(y)));
                }
            }
        }
        min_y
    }

    #[inline(always)]
    pub fn find_all_vertical_wells(&self) -> Vec<(usize, usize, usize)> {
        let heights = self.column_heights();
        let mut wells = Vec::new();

        for x in 0..TOTAL_GRID_WIDTH {
            let h = heights[x];
            if h >= LANE_HEIGHT {
                continue;
            }

            let left_h = if x == 0 { LANE_HEIGHT } else { heights[x - 1] };
            let right_h = if x + 1 >= TOTAL_GRID_WIDTH {
                LANE_HEIGHT
            } else {
                heights[x + 1]
            };

            let wall_h = left_h.min(right_h);
            if wall_h >= h + 2 {
                let depth = wall_h - h;
                let bit = 1u32 << x;
                let has_roof = (h..LANE_HEIGHT).any(|y| (self.rows[y] & bit) != 0);
                if !has_roof {
                    wells.push((x, h, depth));
                }
            }
        }

        wells
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn has_cell(&self, x: usize, y: usize) -> bool {
        if x >= TOTAL_GRID_WIDTH || y >= LANE_HEIGHT {
            return false;
        }
        (self.rows[y] & (1u32 << x)) != 0
    }

    #[inline(always)]
    pub fn find_lane_vertical_wells(&self, lane_id: usize) -> Vec<(usize, usize, usize)> {
        let (min_x, max_x) = lane_x_range(lane_id);
        let heights = self.column_heights();
        let mut wells = Vec::new();

        for x in min_x..=max_x {
            let h = heights[x];
            if h >= LANE_HEIGHT {
                continue;
            }

            let left_h = if x == 0 { LANE_HEIGHT } else { heights[x - 1] };
            let right_h = if x + 1 >= TOTAL_GRID_WIDTH {
                LANE_HEIGHT
            } else {
                heights[x + 1]
            };

            let wall_h = left_h.min(right_h);
            if wall_h >= h + 2 {
                let depth = wall_h - h;
                let bit = 1u32 << x;
                let has_roof = (h..LANE_HEIGHT).any(|y| (self.rows[y] & bit) != 0);
                if !has_roof {
                    wells.push((x, h, depth));
                }
            }
        }

        wells
    }

    #[inline(always)]
    pub fn is_unreachable_alcove(&self, cx: usize, cy: usize) -> bool {
        if cx >= TOTAL_GRID_WIDTH || cy >= LANE_HEIGHT {
            return false;
        }
        let bit = 1u32 << cx;
        if (self.rows[cy] & bit) != 0 {
            return false;
        }

        let roof_opt = (cy + 1..LANE_HEIGHT).find(|&y| (self.rows[y] & bit) != 0);
        if let Some(roof_y) = roof_opt {
            let space_height = roof_y - cy;
            if space_height <= 2 {
                let left_inaccessible = if cx == 0 {
                    true
                } else {
                    (self.rows[cy] & (1u32 << (cx - 1))) != 0
                };
                let right_inaccessible = if cx + 1 >= TOTAL_GRID_WIDTH {
                    true
                } else {
                    (self.rows[cy] & (1u32 << (cx + 1))) != 0
                };

                if left_inaccessible || right_inaccessible {
                    return true;
                }
            }
        }

        false
    }
}
