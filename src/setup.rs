use crate::board::is_border_column;
use crate::config::*;
use crate::game::*;
use bevy::prelude::*;

pub fn setup_game(mut commands: Commands) {
    // 2Dカメラ
    commands.spawn(Camera2d);

    let total_pixel_w = TOTAL_GRID_WIDTH as f32 * (CELL_SIZE + CELL_GAP) - CELL_GAP;
    let total_pixel_h = LANE_HEIGHT as f32 * (CELL_SIZE + CELL_GAP) - CELL_GAP;

    // 背景パネル
    commands.spawn((
        Sprite {
            color: COLOR_BG,
            custom_size: Some(Vec2::new(total_pixel_w + 50.0, total_pixel_h + 90.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -10.0),
    ));

    // 各セルのビジュアル（背景グリッド）を生成
    for y in 0..LANE_HEIGHT {
        for x in 0..TOTAL_GRID_WIDTH {
            let is_border = is_border_column(x);
            let is_spawn_wall = is_border && y >= SPAWN_WALL_MIN_Y;

            let pos = grid_to_world_pos(x as f32, y as f32);

            if is_spawn_wall {
                // 上部投入口の壁（外壁と同様のセパレータ壁）
                commands.spawn((
                    Sprite {
                        color: COLOR_BORDER_WALL,
                        custom_size: Some(Vec2::splat(CELL_SIZE)),
                        ..default()
                    },
                    Transform::from_xyz(pos.x, pos.y, 1.0),
                ));
            } else {
                // 通常セル（下部の境界列も繋がった空間として表示）
                let cell_bg_color = if is_border {
                    COLOR_BORDER_GUIDE
                } else {
                    COLOR_GRID_EMPTY
                };

                commands.spawn((
                    Sprite {
                        color: cell_bg_color,
                        custom_size: Some(Vec2::splat(CELL_SIZE)),
                        ..default()
                    },
                    Transform::from_xyz(pos.x, pos.y, 0.0),
                    BoardCellVisual { x, y },
                ));
            }
        }
    }

    // 4つの投入口スロットエンティティを生成
    for lane_id in 0..LANE_COUNT {
        commands.spawn((
            LaneSlot { id: lane_id },
            LaneSpawnCooldown {
                timer: Timer::from_seconds(0.1 + lane_id as f32 * 0.12, TimerMode::Once),
            },
        ));
    }
}
