use crate::config::LANE_COUNT;

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
#[derive(bevy::prelude::Resource, Debug, Clone)]
pub struct LaneSignalBoard {
    pub signals: [LaneSignal; LANE_COUNT],
}

impl Default for LaneSignalBoard {
    fn default() -> Self {
        let mut signals = [LaneSignal::default(); LANE_COUNT];
        for (i, signal) in signals.iter_mut().enumerate() {
            signal.lane_id = i;
        }
        Self { signals }
    }
}
