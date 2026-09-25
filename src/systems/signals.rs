use crate::board::GlobalBoard;
use crate::game::{FallingTromino, HoleStatus, LanePace, LaneSignal, LaneSignalBoard};
use bevy::prelude::*;

/// 各レーンのシグナル更新システム（人間的な低解像度情報：穴の位置・状態・各自の自己申告ペース・意図）
pub fn update_lane_signals_system(
    board: Res<GlobalBoard>,
    mut signal_board: ResMut<LaneSignalBoard>,
    falling_query: Query<&FallingTromino>,
) {
    for (lane_id, signal) in signal_board.signals.iter_mut().enumerate() {
        // 1. 空白（穴）情報の抽出
        let (hole_x, hole_status) = match board.find_lane_deepest_hole(lane_id) {
            Some((hx, _hy, has_roof)) => {
                if has_roof {
                    (Some(hx), HoleStatus::WaitingForClearance)
                } else {
                    (Some(hx), HoleStatus::ReadyForFill)
                }
            }
            None => (None, HoleStatus::None),
        };

        // 2. 深さ2以上の縦穴（I字で埋めたい溝）の検出
        let vertical_well = board
            .find_lane_vertical_wells(lane_id)
            .into_iter()
            .max_by_key(|&(_, _, depth)| depth);

        // 3. 落下中トミノ（各レーンAI）からの自己申告意図（Intent）および自己決定ペースの抽出
        let falling_opt = falling_query.iter().find(|f| f.lane_id == lane_id);
        let intent_target_x = falling_opt.map(|f| f.target_x);
        let pace = falling_opt.map(|f| f.pace).unwrap_or(LanePace::Normal);

        *signal = LaneSignal {
            lane_id,
            hole_x,
            hole_status,
            vertical_well,
            intent_target_x,
            pace,
        };
    }
}
