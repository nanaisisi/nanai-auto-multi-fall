use bevy::prelude::*;
use crate::config::*;
use crate::tromino::TrominoKind;

#[derive(Component)]
pub struct LaneSlot {
    pub id: usize,
}

#[derive(Component)]
pub struct FallingTromino {
    pub lane_id: usize,
    pub kind: TrominoKind,
    pub current_rotation: usize,
    pub target_rotation: usize,
    pub current_x: f32, // スムーズ補間のため f32 (グリッド座標)
    pub current_y: f32,
    pub target_x: i32,
    pub landing_y: i32,
    pub fall_timer: Timer,
    pub lock_timer: Timer,
    pub rotate_timer: Timer,
}

#[derive(Component)]
pub struct LaneSpawnCooldown {
    pub timer: Timer,
}

#[derive(Component)]
pub struct BoardCellVisual {
    pub x: usize,
    pub y: usize,
}

#[derive(Component)]
pub struct FallingCellVisual;

#[derive(Component)]
pub struct GhostCellVisual;

#[derive(Component)]
pub struct TotalScoreText;

/// グリッド座標 (x, y) から画面のワールド座標 (Vec2) を計算
/// 全体フィールド中央を (0, 0) 付近に配置
pub fn grid_to_world_pos(x: f32, y: f32) -> Vec2 {
    let total_pixel_w = TOTAL_GRID_WIDTH as f32 * (CELL_SIZE + CELL_GAP) - CELL_GAP;
    let total_pixel_h = LANE_HEIGHT as f32 * (CELL_SIZE + CELL_GAP) - CELL_GAP;

    let origin_x = -total_pixel_w / 2.0 + CELL_SIZE / 2.0;
    let origin_y = -total_pixel_h / 2.0 + CELL_SIZE / 2.0;

    let world_x = origin_x + x * (CELL_SIZE + CELL_GAP);
    let world_y = origin_y + y * (CELL_SIZE + CELL_GAP);

    Vec2::new(world_x, world_y)
}
