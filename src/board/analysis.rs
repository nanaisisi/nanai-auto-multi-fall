use super::grid::GlobalBoard;
use super::lane_x_range;
use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};

impl GlobalBoard {
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

    /// 盤面全体で最も高い位置にある穴（最上位の空白）のy座標を返す
    #[allow(dead_code, clippy::needless_range_loop)]
    pub fn highest_hole_y(&self) -> Option<usize> {
        let mut max_y = None;
        for x in 0..TOTAL_GRID_WIDTH {
            let mut roof_found = false;
            for y in (0..LANE_HEIGHT).rev() {
                if self.cells[y][x].is_some() {
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
    #[allow(clippy::needless_range_loop)]
    pub fn has_hole_below_in_row(&self, y: usize) -> bool {
        if y == 0 {
            return false;
        }
        for x in 0..TOTAL_GRID_WIDTH {
            let has_roof = (y..LANE_HEIGHT).any(|ry| self.cells[ry][x].is_some());
            if has_roof {
                for uy in 0..y {
                    if self.cells[uy][x].is_none() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 盤面全体で最も低い位置にある穴（最下層の空白）のy座標を返す
    #[allow(dead_code, clippy::needless_range_loop)]
    pub fn lowest_hole_y(&self) -> Option<usize> {
        let mut min_y = None;
        for x in 0..TOTAL_GRID_WIDTH {
            let mut roof_found = false;
            for y in (0..LANE_HEIGHT).rev() {
                if self.cells[y][x].is_some() {
                    roof_found = true;
                } else if roof_found {
                    min_y = Some(min_y.map_or(y, |prev: usize| prev.min(y)));
                }
            }
        }
        min_y
    }

    /// 穴の個数を計算
    #[allow(clippy::needless_range_loop)]
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

    /// 指定レーン範囲内の穴（最深の空白）の座標 (x, y) および頭上にブロックが被さっているかを返す
    #[allow(clippy::needless_range_loop)]
    pub fn find_lane_deepest_hole(&self, lane_id: usize) -> Option<(usize, usize, bool)> {
        let (min_x, max_x) = lane_x_range(lane_id);

        let mut deepest_hole: Option<(usize, usize, bool)> = None;

        for x in min_x..=max_x {
            let mut roof_exists = false;
            for y in (0..LANE_HEIGHT).rev() {
                if self.cells[y][x].is_some() {
                    roof_exists = true;
                } else if roof_exists {
                    match deepest_hole {
                        None => deepest_hole = Some((x, y, true)),
                        Some((_, prev_y, _)) if y < prev_y => deepest_hole = Some((x, y, true)),
                        _ => {}
                    }
                }
            }
        }

        // もし屋根が被さっている穴がなければ、最下層付近で未配置のマス（隣の列より凹んでいる谷）をチェック
        if deepest_hole.is_none() {
            let heights = self.column_heights();
            let avg_height: f32 = (min_x..=max_x).map(|x| heights[x] as f32).sum::<f32>()
                / (max_x - min_x + 1) as f32;
            for x in min_x..=max_x {
                if (heights[x] as f32) < avg_height - 1.0 {
                    return Some((x, heights[x], false));
                }
            }
        }

        deepest_hole
    }

    /// レーン内の深さ2以上の縦穴（左右が壁やブロックで囲まれ、上方が空いている溝）を検出
    /// 戻り値: (x座標, 穴の底y座標, 穴の深さ)
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
                let has_roof = (h..LANE_HEIGHT).any(|y| self.cells[y][x].is_some());
                if !has_roof {
                    wells.push((x, h, depth));
                }
            }
        }

        wells
    }

    /// フィールド全体における深さ2以上の縦穴をすべて検出
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
                let has_roof = (h..LANE_HEIGHT).any(|y| self.cells[y][x].is_some());
                if !has_roof {
                    wells.push((x, h, depth));
                }
            }
        }

        wells
    }

    /// 指定された空洞マス (cx, cy) が埋めることの不可能な空間かを判定
    pub fn is_unreachable_alcove(&self, cx: usize, cy: usize) -> bool {
        if cx >= TOTAL_GRID_WIDTH || cy >= LANE_HEIGHT || self.cells[cy][cx].is_some() {
            return false;
        }

        let roof_opt = (cy + 1..LANE_HEIGHT).find(|&y| self.cells[y][cx].is_some());
        if let Some(roof_y) = roof_opt {
            let space_height = roof_y - cy;
            if space_height <= 2 {
                let left_inaccessible = if cx == 0 {
                    true
                } else {
                    self.cells[cy][cx - 1].is_some()
                };
                let right_inaccessible = if cx + 1 >= TOTAL_GRID_WIDTH {
                    true
                } else {
                    self.cells[cy][cx + 1].is_some()
                };

                if left_inaccessible || right_inaccessible {
                    return true;
                }
            }
        }

        false
    }

    /// 対象ターゲット行における自レーンのブロック充足率 (0.0〜1.0) を算出
    #[allow(clippy::needless_range_loop)]
    pub fn lane_fill_ratio_at_target_line(&self, lane_id: usize) -> f32 {
        let (min_x, max_x) = lane_x_range(lane_id);
        let lane_w = max_x - min_x + 1;

        let mut best_target_y = 0;
        let mut max_overall_filled = 0;
        for y in 0..5.min(LANE_HEIGHT) {
            let filled_count = (0..TOTAL_GRID_WIDTH)
                .filter(|&x| self.cells[y][x].is_some())
                .count();
            if filled_count > max_overall_filled {
                max_overall_filled = filled_count;
                best_target_y = y;
            }
        }

        if max_overall_filled == 0 {
            return 0.0;
        }

        let my_lane_filled = (min_x..=max_x)
            .filter(|&x| self.cells[best_target_y][x].is_some())
            .count();

        my_lane_filled as f32 / lane_w as f32
    }
}
