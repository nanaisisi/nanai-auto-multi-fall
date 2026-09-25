use super::signals::LanePace;
use crate::config::{CELL_GAP, CELL_SIZE, LANE_HEIGHT, TOTAL_GRID_WIDTH};
use crate::tromino::TrominoKind;
use bevy::prelude::*;

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
    pub is_on_ground: bool,
    pub lock_resets_left: usize,
    pub pace: LanePace,
    pub waypoints: Vec<(i32, i32)>,
    /// 目標着地点を決定した時点のボードバージョン（盤面変化検知用）
    pub planned_board_version: u64,
    /// 状況変化や予期せぬ事態に応じた定期的再計算タイマー
    pub replan_timer: Timer,
}

impl FallingTromino {
    /// 新規トミノのスポーン時生成ヘルパー
    #[allow(clippy::too_many_arguments)]
    pub fn new_spawned(
        lane_id: usize,
        kind: TrominoKind,
        initial_rotation: usize,
        target_rotation: usize,
        start_x: f32,
        start_y: f32,
        target_x: i32,
        landing_y: i32,
        fall_interval: f32,
        pace: LanePace,
        waypoints: Vec<(i32, i32)>,
        board_version: u64,
    ) -> Self {
        Self {
            lane_id,
            kind,
            current_rotation: initial_rotation,
            target_rotation,
            current_x: start_x,
            current_y: start_y,
            target_x,
            landing_y,
            fall_timer: Timer::from_seconds(fall_interval, TimerMode::Repeating),
            lock_timer: Timer::from_seconds(crate::config::LOCK_DELAY, TimerMode::Once),
            rotate_timer: Timer::from_seconds(0.08, TimerMode::Repeating),
            is_on_ground: false,
            lock_resets_left: crate::config::MAX_LOCK_RESETS,
            pace,
            waypoints,
            planned_board_version: board_version,
            replan_timer: Timer::from_seconds(0.60, TimerMode::Repeating),
        }
    }
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
