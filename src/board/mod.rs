use crate::config::{LANE_HEIGHT, LANE_WIDTH, SPAWN_WALL_MIN_Y, TOTAL_GRID_WIDTH, player_color};
use crate::tromino::TrominoKind;
use bevy::prelude::*;

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

#[derive(Resource, Debug, Clone)]
pub struct GlobalBoard {
    // [y][x] : y=0 が最下段、y=LANE_HEIGHT-1 が最上段
    pub cells: [[Option<Color>; TOTAL_GRID_WIDTH]; LANE_HEIGHT],
    pub score: u32,
    pub lines_cleared: u32,
    pub game_over: bool,
    pub lane_stuck: [bool; crate::config::LANE_COUNT],
    /// 盤面の固定・消去が発生するたびにインクリメントされるバージョン番号
    pub board_version: u64,
}

impl Default for GlobalBoard {
    fn default() -> Self {
        Self {
            cells: [[None; TOTAL_GRID_WIDTH]; LANE_HEIGHT],
            score: 0,
            lines_cleared: 0,
            game_over: false,
            lane_stuck: [false; crate::config::LANE_COUNT],
            board_version: 0,
        }
    }
}

impl GlobalBoard {
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.cells = [[None; TOTAL_GRID_WIDTH]; LANE_HEIGHT];
        self.game_over = false;
        self.lane_stuck = [false; crate::config::LANE_COUNT];
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
    #[allow(dead_code)]
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

    /// 盤面全体で最も低い位置にある穴（最下層の空白）のy座標を返す
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
                    // 空白を発見。より低い位置 (y が小さい) の穴を優先
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
                    // 明らかに凹んでいる（今すぐ埋められる）場所
                    return Some((x, heights[x], false)); // 屋根なし (ReadyForFill)
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
            // x列の現在の高さ（一番上のブロックのy+1、ブロックがなければ0）
            let h = heights[x];
            if h >= LANE_HEIGHT {
                continue;
            }

            // 左右の壁/ブロックの高さを判定
            // xが0またはTOTAL_GRID_WIDTH-1なら盤面の外壁として扱う
            let left_h = if x == 0 { LANE_HEIGHT } else { heights[x - 1] };
            let right_h = if x + 1 >= TOTAL_GRID_WIDTH {
                LANE_HEIGHT
            } else {
                heights[x + 1]
            };

            // 左右両方の壁のうち低い方と、現在列の高さの差が「縦穴の深さ」
            let wall_h = left_h.min(right_h);
            if wall_h >= h + 2 {
                let depth = wall_h - h;
                // 屋根（hより上のブロック）がないか確認
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

    /// 指定された空洞マス (cx, cy) が、幅1マスの縦穴の下または奥にあり、
    /// かつ高さが2マス以下（I字は入れず、L字は幅1マスの縦穴を通れない）ため、
    /// トミノで埋めることが原理的に不可能な「どうしようもない空間」かを判定
    pub fn is_unreachable_alcove(&self, cx: usize, cy: usize) -> bool {
        if cx >= TOTAL_GRID_WIDTH || cy >= LANE_HEIGHT || self.cells[cy][cx].is_some() {
            return false;
        }

        // 1. 真上に屋根（オーバーハング）があるか
        let roof_opt = (cy + 1..LANE_HEIGHT).find(|&y| self.cells[y][cx].is_some());
        if let Some(roof_y) = roof_opt {
            // この空洞の直上の空間の高さ (roof_y - cy) が 2 以下であれば、
            // 縦向きI字（高さ3）は物理的に進入できない
            let space_height = roof_y - cy;
            if space_height <= 2 {
                // 左右どちらかから横スライドで入れるか？
                // 左右の入口が「幅1の縦穴」または「壁/ブロック」で塞がれているか判定
                // 左側チェック
                let left_inaccessible = if cx == 0 {
                    true
                } else {
                    // 左の列のブロック高さが roof_y 以上なら横から入れない
                    // または左が幅1の縦穴でL字（幅2）が進入できない
                    self.cells[cy][cx - 1].is_some()
                };
                // 右側チェック
                let right_inaccessible = if cx + 1 >= TOTAL_GRID_WIDTH {
                    true
                } else {
                    self.cells[cy][cx + 1].is_some()
                };

                // もし左右とも壁・ブロックで直接横から入れず、縦穴を介してしかアクセスできない場合
                if left_inaccessible || right_inaccessible {
                    return true;
                }
            }
        }

        false
    }

    /// 対象ターゲット行（最もブロックが揃っている下層段）における自レーンのブロック充足率 (0.0〜1.0) を算出
    #[allow(clippy::needless_range_loop)]
    pub fn lane_fill_ratio_at_target_line(&self, lane_id: usize) -> f32 {
        let (min_x, max_x) = lane_x_range(lane_id);
        let lane_w = max_x - min_x + 1;

        // 全体で最も揃っている（あと数個で消えそうな）下位行を探す
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

        // 列番号インデックスヘッダー (10の位 & 1の位)
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
                        out.push('|'); // 上部投入壁
                    } else if self.cells[y][x].is_some() {
                        out.push('#'); // 境界列に置かれたブロック
                    } else {
                        out.push(':'); // 下部連結空間の境界ガイド
                    }
                } else if self.cells[y][x].is_some() {
                    out.push('#'); // 通常ブロック
                } else {
                    // 空白マス：頭上にブロックがあれば穴 (o)、なければ空 (・)
                    let is_hole = (y + 1..LANE_HEIGHT).any(|hy| self.cells[hy][x].is_some());
                    if is_hole {
                        out.push('.'); // 穴（空洞）
                    } else {
                        out.push(' '); // オープンな空きマス
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

        // 各レーンごとの番号表示
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

pub mod compact;
pub use compact::CompactBoard;
