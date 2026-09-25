use crate::ai::PredictedPlacement;
use crate::config::GameSettings;
use crate::game::{FallingTromino, LanePace};
use bevy::prelude::*;

/// 落下中トミノの残り着地予想時間 (ETA) を計算し、ETA順にソートされた着地予測リストを生成
pub fn collect_sorted_predictions(
    falling_query: &Query<&FallingTromino>,
    settings: &GameSettings,
    lines_cleared: u32,
) -> Vec<PredictedPlacement> {
    let base_interval = settings.current_base_fall_interval(lines_cleared);

    let mut falling_etas: Vec<(f32, PredictedPlacement)> = falling_query
        .iter()
        .map(|falling| {
            let dy = (falling.current_y - falling.landing_y as f32).max(0.0);
            let interval = match falling.pace {
                LanePace::SoftDrop => base_interval * settings.soft_drop_multiplier,
                LanePace::Normal => base_interval,
            };
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
pub fn collect_air_obstacles(falling_query: &Query<&FallingTromino>) -> Vec<(i32, i32)> {
    let mut air_obstacles = Vec::new();
    for falling in falling_query.iter() {
        let offsets = falling.kind.cell_offsets(falling.current_rotation);
        for (dx, dy) in &offsets {
            air_obstacles.push((
                falling.current_x.round() as i32 + dx,
                falling.current_y.round() as i32 + dy,
            ));
        }
    }
    air_obstacles
}
