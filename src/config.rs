use bevy::prelude::*;

// 標準8スロット設定
pub const LANE_COUNT: usize = 8;
pub const LANE_WIDTH: usize = 3;
pub const BORDER_WIDTH: usize = 1;

// 全体幅 = (8 * 3) + (7 * 1) = 31 セル
pub const TOTAL_GRID_WIDTH: usize = (LANE_COUNT * LANE_WIDTH) + ((LANE_COUNT - 1) * BORDER_WIDTH);

// 高さ設定: 高さ 4 (上部投入壁) + 15 (下部連結プレイ空間) = 19
pub const SPAWN_WALL_HEIGHT: usize = 4;
pub const PLAYABLE_HEIGHT: usize = 15;
pub const LANE_HEIGHT: usize = SPAWN_WALL_HEIGHT + PLAYABLE_HEIGHT; // 19

// 上部投入壁の開始Y (19 - 4 = 15) -> y >= 15 が壁、y < 15 が連結空間
pub const SPAWN_WALL_MIN_Y: usize = LANE_HEIGHT - SPAWN_WALL_HEIGHT; // 15

// 31セル幅がきれいに1280x720画面に収まるようセルサイズを微調整 (20.0px + 2.0px gap)
// 全体幅: 31 * 22 - 2 = 680px (余裕で画面内に美しく収まる)
// 全体高: 19 * 22 - 2 = 416px
pub const CELL_SIZE: f32 = 20.0;
pub const CELL_GAP: f32 = 2.0;

pub const COLOR_BG: Color = Color::srgb(0.06, 0.07, 0.10);
pub const COLOR_BORDER_WALL: Color = Color::srgb(0.35, 0.38, 0.48); // 上部投入口の壁
pub const COLOR_BORDER_GUIDE: Color = Color::srgb(0.13, 0.15, 0.21); // 下部の境界列グリッド背景
pub const COLOR_GRID_EMPTY: Color = Color::srgb(0.09, 0.11, 0.15);

// 8人プレイ用の鮮やかなネオンカラーパレット
pub const PLAYER_COLORS: [Color; 8] = [
    Color::srgb(0.12, 0.85, 0.98), // P1: Neon Cyan
    Color::srgb(0.98, 0.26, 0.65), // P2: Neon Magenta
    Color::srgb(0.30, 0.95, 0.42), // P3: Neon Lime Green
    Color::srgb(1.00, 0.68, 0.15), // P4: Neon Amber / Gold
    Color::srgb(0.70, 0.35, 0.98), // P5: Neon Purple / Violet
    Color::srgb(0.15, 0.65, 1.00), // P6: Deep Sky Blue
    Color::srgb(1.00, 0.35, 0.35), // P7: Neon Coral / Red
    Color::srgb(0.20, 0.95, 0.82), // P8: Bright Teal
];

pub fn player_color(player_id: usize) -> Color {
    PLAYER_COLORS[player_id % PLAYER_COLORS.len()]
}

pub fn player_ghost_color(player_id: usize) -> Color {
    let base = player_color(player_id);
    let srgba = base.to_srgba();
    Color::srgba(srgba.red, srgba.green, srgba.blue, 0.28)
}

pub const LOCK_DELAY: f32 = 0.40; // 接地してから固定されるまでの猶予時間
pub const MAX_LOCK_RESETS: usize = 4; // 接地猶予時間内のスライド・移動によるロックリセット最大回数

#[derive(Resource)]
pub struct GameSettings {
    pub initial_fall_interval: f32, // 初期基準落下間隔（秒）
    pub min_fall_interval: f32,     // 消去加速の上限速度（最短落下間隔）
    pub spawn_delay: f32,
    pub soft_drop_multiplier: f32, // 下キー入力（ソフトドロップ）時の落下間隔倍率
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            initial_fall_interval: 0.35, // 初期はゆったり（約2.8マス/秒）見やすく快適なペース
            min_fall_interval: 0.04,     // 最高速・困難時（25マス/秒）
            spawn_delay: 0.12,
            soft_drop_multiplier: 0.35, // 下キー入力時は高速落下
        }
    }
}

impl GameSettings {
    /// 消去ライン数に比例して加速するシステム基準落下間隔
    /// 100消し程度で最高速度・困難ゾーン（0.04s〜0.06s）に到達するよう設計
    pub fn current_base_fall_interval(&self, lines_cleared: u32) -> f32 {
        // 100ライン消去で約 0.30秒 短縮し、最高難易度（min_fall_interval）へ到達
        let speedup = lines_cleared as f32 * 0.0031;
        (self.initial_fall_interval - speedup).max(self.min_fall_interval)
    }
}
