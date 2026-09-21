use bevy::prelude::*;
use crate::board::{lane_x_range, GlobalBoard};
use crate::config::*;
use crate::game::*;

pub fn setup_ui(mut commands: Commands) {
    let total_pixel_h = LANE_HEIGHT as f32 * (CELL_SIZE + CELL_GAP) - CELL_GAP;
    let header_y = (total_pixel_h / 2.0) + 25.0;

    // 各投入口（スロット）のラベル表示（プレイヤーカラー対応）
    for lane_id in 0..LANE_COUNT {
        let (min_x, max_x) = lane_x_range(lane_id);
        let center_x = (min_x + max_x) as f32 / 2.0;
        let world_pos = grid_to_world_pos(center_x, (LANE_HEIGHT - 1) as f32);

        let color = player_color(lane_id);

        commands.spawn((
            Text2d::new(format!("P{}\nS{}", lane_id + 1, lane_id + 1)),
            TextFont {
                font_size: bevy::text::FontSize::Px(10.5),
                ..default()
            },
            TextColor(color),
            TextLayout::default().with_justify(Justify::Center),
            Transform::from_xyz(world_pos.x, header_y + 5.0, 10.0),
        ));
    }

    // 全体スコア表示
    commands.spawn((
        Text2d::new("NANAI AUTO MULTI-FALL (WIDE FIELD)\nTOTAL SCORE: 0  |  TOTAL LINES: 0"),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.35, 0.9, 1.0)),
        TextLayout::default().with_justify(Justify::Center),
        Transform::from_xyz(0.0, header_y + 40.0, 10.0),
        TotalScoreText,
    ));
}

pub fn update_ui_system(
    board: Res<GlobalBoard>,
    mut total_text_query: Query<&mut Text2d, With<TotalScoreText>>,
) {
    for mut text in total_text_query.iter_mut() {
        **text = format!(
            "NANAI AUTO MULTI-FALL (WIDE FIELD)\nTOTAL SCORE: {}  |  TOTAL LINES: {}",
            board.score, board.lines_cleared
        );
    }
}
