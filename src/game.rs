use bevy::prelude::*;
use crate::config::*;
use crate::tromino::TrominoKind;

#[derive(Component)]
pub struct LaneSlot {
    pub id: usize,
}

/// 人間のような曖昧・限定的なレーン間コミュニケーションシグナル
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleStatus {
    None,
    /// 上の段にブロックが被さっており、消去されるのを待機している状態（上を塞がないでほしい合図）
    WaitingForClearance,
    /// 上のブロックが消えて穴が露出し、横や上から差し込める状態
    ReadyForFill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanePace {
    /// 下キー入力（ソフトドロップ）: 自レーンの遅れを取り戻すため自発的に急降下
    SoftDrop,
    /// 自然落下: システム側の加速度そのままのスピードで落下
    Normal,
}


#[derive(Debug, Clone, Copy)]
pub struct LaneSignal {
    pub lane_id: usize,
    /// 空白（穴）の存在列（0..TOTAL_GRID_WIDTH）
    pub hole_x: Option<usize>,
    pub hole_status: HoleStatus,
    /// 深さ2以上の縦穴情報: (x座標, 穴の底y座標, 穴の深さ)
    pub vertical_well: Option<(usize, usize, usize)>,
    /// 配置予定の概算列 (人間が「ここ置くよ！」と宣言する曖昧な目安: ±1程度のブレを想定)
    pub intent_target_x: Option<i32>,
    /// 自レーンのペース
    pub pace: LanePace,
}

impl Default for LaneSignal {
    fn default() -> Self {
        Self {
            lane_id: 0,
            hole_x: None,
            hole_status: HoleStatus::None,
            vertical_well: None,
            intent_target_x: None,
            pace: LanePace::Normal,
        }
    }
}

/// 全レーンの限定シグナル共有ボード（低解像度な連絡板）
#[derive(Resource, Debug, Clone)]
pub struct LaneSignalBoard {
    pub signals: [LaneSignal; LANE_COUNT],
}

impl Default for LaneSignalBoard {
    fn default() -> Self {
        let mut signals = [LaneSignal::default(); LANE_COUNT];
        for i in 0..LANE_COUNT {
            signals[i].lane_id = i;
        }
        Self { signals }
    }
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
