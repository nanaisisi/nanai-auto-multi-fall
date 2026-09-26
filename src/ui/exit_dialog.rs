use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct ExitDialogState {
    pub is_open: bool,
}

#[derive(Component)]
pub struct OpenExitDialogButton;

#[derive(Component)]
pub struct ConfirmExitButton;

#[derive(Component)]
pub struct CancelExitButton;

#[derive(Component)]
pub struct ExitConfirmModal;

pub fn setup_exit_dialog(mut commands: Commands) {
    // 右上（常時・プレイ途中および終了後）の「終了」ボタン（盤面を遮らない配置）
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
                        width: Val::Px(96.0),
                        height: Val::Px(34.0),
                        border: UiRect::all(Val::Px(1.5)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.5, 0.2, 0.25)),
                    BackgroundColor(Color::srgba(0.2, 0.08, 0.1, 0.85)),
                    OpenExitDialogButton,
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

    // 終了確認モーダルダイアログ（初期状態は非表示）
    // 盤面が確認できるように半透明バックドロップを採用
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ExitConfirmModal,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(32.0), Val::Px(24.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        row_gap: Val::Px(16.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.10, 0.15, 0.95)),
                    BorderColor::all(Color::srgb(0.7, 0.3, 0.35)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("[ 一時停止中 ]"),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.8, 0.4)),
                    ));
                    panel.spawn((
                        Text::new("ゲームを終了しますか？"),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    // 確認ボタン行（終了する / 続ける）
                    panel
                        .spawn((Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(16.0),
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },))
                        .with_children(|row| {
                            // 本当に終了するボタン
                            row.spawn((
                                Button,
                                Node {
                                    width: Val::Px(110.0),
                                    height: Val::Px(36.0),
                                    border: UiRect::all(Val::Px(1.5)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border_radius: BorderRadius::all(Val::Px(6.0)),
                                    ..default()
                                },
                                BorderColor::all(Color::srgb(0.9, 0.3, 0.35)),
                                BackgroundColor(Color::srgba(0.45, 0.12, 0.16, 0.95)),
                                ConfirmExitButton,
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new("終了する"),
                                    TextFont {
                                        font_size: bevy::text::FontSize::Px(14.0),
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                            });

                            // キャンセルボタン（続ける）
                            row.spawn((
                                Button,
                                Node {
                                    width: Val::Px(110.0),
                                    height: Val::Px(36.0),
                                    border: UiRect::all(Val::Px(1.5)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border_radius: BorderRadius::all(Val::Px(6.0)),
                                    ..default()
                                },
                                BorderColor::all(Color::srgb(0.35, 0.4, 0.5)),
                                BackgroundColor(Color::srgba(0.18, 0.22, 0.28, 0.9)),
                                CancelExitButton,
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new("続ける"),
                                    TextFont {
                                        font_size: bevy::text::FontSize::Px(14.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.85, 0.9, 0.95)),
                                ));
                            });
                        });
                });
        });
}

type OpenDialogQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (Changed<Interaction>, With<OpenExitDialogButton>),
>;

type ConfirmExitQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<ConfirmExitButton>),
>;

type CancelExitQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<CancelExitButton>),
>;

/// 終了ボタン、確認モーダル、およびゲーム一時停止（Time<Virtual>）の連動システム
#[allow(clippy::too_many_arguments)]
pub fn exit_dialog_interaction_system(
    mut exit_events: MessageWriter<AppExit>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<Time<Virtual>>,
    mut dialog_state: ResMut<ExitDialogState>,
    mut modal_query: Query<&mut Node, With<ExitConfirmModal>>,
    mut open_btn_query: OpenDialogQuery,
    mut confirm_btn_query: ConfirmExitQuery,
    mut cancel_btn_query: CancelExitQuery,
) {
    let mut should_open = false;
    let mut should_close = false;
    let mut should_exit = false;

    // 1. キーボード操作
    if keyboard_input.just_pressed(KeyCode::Escape) {
        if dialog_state.is_open {
            should_close = true;
        } else {
            should_open = true;
        }
    } else if keyboard_input.just_pressed(KeyCode::KeyQ) {
        should_open = true;
    }

    // 2. 右上終了ボタン押下
    for (interaction, mut bg_color, mut border_color) in open_btn_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                should_open = true;
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

    // 3. モーダル内の「終了する」ボタン
    for (interaction, mut bg_color) in confirm_btn_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                should_exit = true;
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(Color::srgba(0.65, 0.15, 0.2, 0.98));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(Color::srgba(0.45, 0.12, 0.16, 0.95));
            }
        }
    }

    // 4. モーダル内の「続ける」ボタン
    for (interaction, mut bg_color) in cancel_btn_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                should_close = true;
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(Color::srgba(0.28, 0.34, 0.42, 0.95));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(Color::srgba(0.18, 0.22, 0.28, 0.9));
            }
        }
    }

    if should_exit {
        exit_events.write(AppExit::Success);
        return;
    }

    if should_open && !dialog_state.is_open {
        dialog_state.is_open = true;
        time.pause();
    } else if should_close && dialog_state.is_open {
        dialog_state.is_open = false;
        time.unpause();
    }

    // モーダルの表示/非表示を更新
    for mut node in modal_query.iter_mut() {
        let target_display = if dialog_state.is_open {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != target_display {
            node.display = target_display;
        }
    }
}
