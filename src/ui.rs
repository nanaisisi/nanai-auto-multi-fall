use crate::board::{GlobalBoard, lane_x_range};
use crate::config::*;
use crate::game::*;
use bevy::prelude::*;

#[derive(Component)]
pub struct LaneLabel {
    pub lane_id: usize,
}

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
            LaneLabel { lane_id },
        ));
    }

    // 全体スコア・ライン消去数・速度表示
    commands.spawn((
        Text2d::new("NANAI AUTO MULTI-FALL\nLINES CLEARED: 0 / 100  |  SCORE: 0  |  SPEED: 1.0x"),
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
    settings: Res<GameSettings>,
    mut total_text_query: Query<&mut Text2d, (With<TotalScoreText>, Without<LaneLabel>)>,
    mut lane_label_query: Query<(&LaneLabel, &mut Text2d, &mut TextColor), Without<TotalScoreText>>,
) {
    for (lane_label, mut text, mut text_color) in lane_label_query.iter_mut() {
        let is_stuck = board.lane_stuck[lane_label.lane_id];
        if is_stuck {
            **text = format!("P{}\n[STUCK]", lane_label.lane_id + 1);
            text_color.0 = Color::srgb(1.0, 0.25, 0.25);
        } else {
            **text = format!("P{}\nS{}", lane_label.lane_id + 1, lane_label.lane_id + 1);
            text_color.0 = player_color(lane_label.lane_id);
        }
    }

    let cur_interval = settings.current_base_fall_interval(board.lines_cleared);
    let speed_multiplier = settings.initial_fall_interval / cur_interval;

    for mut text in total_text_query.iter_mut() {
        if board.game_over {
            **text = format!(
                "--- GAME OVER (ALL LANES STUCK) ---\nLINES CLEARED: {}  |  FINAL SCORE: {}",
                board.lines_cleared, board.score
            );
        } else {
            let stuck_count = board.lane_stuck.iter().filter(|&&s| s).count();
            if stuck_count > 0 {
                **text = format!(
                    "NANAI AUTO MULTI-FALL  [STUCK: {}/{}]\n★ LINES CLEARED: {}  |  SCORE: {}  |  SPEED: {:.1}x",
                    stuck_count, LANE_COUNT, board.lines_cleared, board.score, speed_multiplier
                );
            } else {
                **text = format!(
                    "NANAI AUTO MULTI-FALL\n★ LINES CLEARED: {}  |  SCORE: {}  |  SPEED: {:.1}x",
                    board.lines_cleared, board.score, speed_multiplier
                );
            }
        }
    }
}
