use crate::ai::PredictedPlacement;
use crate::config::GameSettings;
use crate::game::FallingTromino;
use bevy::prelude::*;

/// 落下中トミノの残り着地予想時間 (ETA) を計算し、ETA順にソートされた着地予測リストを生成
pub fn collect_sorted_predictions<'a>(
    fallings: impl IntoIterator<Item = &'a FallingTromino>,
    settings: &GameSettings,
    lines_cleared: u32,
) -> Vec<PredictedPlacement> {
    let base_interval = settings.current_base_fall_interval(lines_cleared);

    let mut falling_etas: Vec<(f32, PredictedPlacement)> = fallings
        .into_iter()
        .map(|falling| {
            let dy = (falling.current_y - falling.landing_y as f32).max(0.0);
            let interval = settings.calculate_fall_interval(base_interval, falling.pace);
            let eta = dy * interval;

            (
                eta,
                PredictedPlacement {
                    player_id: falling.lane_id,
                    kind: falling.kind,
                    rotation: falling.target_rotation,
                    target_x: falling.target_x,
                    landing_y: falling.landing_y,
                },
            )
        })
        .collect();

    falling_etas.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    falling_etas.into_iter().map(|(_, p)| p).collect()
}

/// 落下中トミノが空中で占有しているセル座標（動的空中障害物）を収集
pub fn collect_air_obstacles<'a>(
    fallings: impl IntoIterator<Item = &'a FallingTromino>,
) -> Vec<(i32, i32)> {
    fallings
        .into_iter()
        .flat_map(|falling| falling.occupied_cells())
        .collect()
}
