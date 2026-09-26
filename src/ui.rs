use crate::board::{GlobalBoard, lane_x_range};
use crate::config::*;
use crate::game::*;
use bevy::prelude::*;

#[derive(Component)]
pub struct LaneLabel {
    pub lane_id: usize,
}

#[derive(Component)]
pub struct ExitButton;

#[derive(Component)]
pub struct GameOverExitBanner;

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

    // 右上（常時・プレイ途中）の「終了」ボタン
    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            right: Val::Px(16.0),
            ..default()
        },))
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(90.0),
                        height: Val::Px(36.0),
                        border: UiRect::all(Val::Px(1.5)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.5, 0.2, 0.25)),
                    BackgroundColor(Color::srgba(0.2, 0.08, 0.1, 0.85)),
                    ExitButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("終了 (ESC)"),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.8, 0.8)),
                    ));
                });
        });

    // ゲームオーバー時用の中央大型終了バナー（初期状態は非表示）
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(0.0),
                right: Val::Percent(0.0),
                top: Val::Percent(0.0),
                bottom: Val::Percent(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            GameOverExitBanner,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(24.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(12.0)),
                        row_gap: Val::Px(16.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.09, 0.14, 0.94)),
                    BorderColor::all(Color::srgb(0.85, 0.25, 0.3)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("--- GAME OVER ---"),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(24.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.3, 0.3)),
                    ));
                    panel
                        .spawn((
                            Button,
                            Node {
                                width: Val::Px(180.0),
                                height: Val::Px(44.0),
                                border: UiRect::all(Val::Px(1.5)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.9, 0.3, 0.35)),
                            BackgroundColor(Color::srgba(0.4, 0.12, 0.15, 0.9)),
                            ExitButton,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("ウインドウを閉じる"),
                                TextFont {
                                    font_size: bevy::text::FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                });
        });
}

pub fn update_ui_system(
    board: Res<GlobalBoard>,
    settings: Res<GameSettings>,
    mut total_text_query: Query<&mut Text2d, (With<TotalScoreText>, Without<LaneLabel>)>,
    mut lane_label_query: Query<(&LaneLabel, &mut Text2d, &mut TextColor), Without<TotalScoreText>>,
    mut game_over_banner_query: Query<&mut Node, With<GameOverExitBanner>>,
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

    // ゲームオーバー時に大型終了バナーを表示
    for mut node in game_over_banner_query.iter_mut() {
        if board.game_over {
            if node.display != Display::Flex {
                node.display = Display::Flex;
            }
        } else if node.display != Display::None {
            node.display = Display::None;
        }
    }
}

type ExitButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (Changed<Interaction>, With<ExitButton>),
>;

/// 終了ボタン押下およびEscキー/Qキー入力によるウィンドウ終了処理
pub fn exit_button_interaction_system(
    mut exit_events: MessageWriter<AppExit>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut interaction_query: ExitButtonInteractionQuery,
) {
    // 1. キーボードショートカット（Esc または Q）での終了
    if keyboard_input.just_pressed(KeyCode::Escape) || keyboard_input.just_pressed(KeyCode::KeyQ) {
        exit_events.write(AppExit::Success);
        return;
    }

    // 2. ボタン操作（Hover/Click）での終了
    for (interaction, mut bg_color, mut border_color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg_color = BackgroundColor(Color::srgb(0.8, 0.1, 0.15));
                exit_events.write(AppExit::Success);
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(Color::srgba(0.35, 0.1, 0.15, 0.95));
                *border_color = BorderColor::all(Color::srgb(0.95, 0.4, 0.45));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(Color::srgba(0.2, 0.08, 0.1, 0.85));
                *border_color = BorderColor::all(Color::srgb(0.5, 0.2, 0.25));
            }
        }
    }
}
