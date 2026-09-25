use crate::board::{GlobalBoard, is_border_column};
use crate::config::*;
use crate::game::*;
use bevy::prelude::*;

type DynamicVisualQuery<'w, 's> =
    Query<'w, 's, Entity, Or<(With<FallingCellVisual>, With<GhostCellVisual>)>>;

/// 固定済みセル、落下中セル、ゴースト（着地予測）を描画・更新するシステム
pub fn render_system(
    mut commands: Commands,
    board: Res<GlobalBoard>,
    mut cell_visual_query: Query<(&BoardCellVisual, &mut Sprite)>,
    falling_query: Query<&FallingTromino>,
    dynamic_visual_query: DynamicVisualQuery,
) {
    // 1. 固定グリッドセルの色をボード状態に同期（各プレイヤー固有色）
    for (cell_vis, mut sprite) in cell_visual_query.iter_mut() {
        if let Some(color) = board.cells[cell_vis.y][cell_vis.x] {
            sprite.color = color;
        } else {
            sprite.color = if is_border_column(cell_vis.x) {
                COLOR_BORDER_GUIDE
            } else {
                COLOR_GRID_EMPTY
            };
        }
    }

    // 2. 前フレームの動的セル（落下中、ゴースト）をクリーンアップ
    for entity in dynamic_visual_query.iter() {
        commands.entity(entity).despawn();
    }

    // 3. 落下中トミノとゴーストの生成（プレイヤー固有色）
    for falling in falling_query.iter() {
        let player_col = player_color(falling.lane_id);
        let ghost_col = player_ghost_color(falling.lane_id);

        // ゴースト（着地予想位置・目標回転）描画
        let target_offsets = falling.kind.cell_offsets(falling.target_rotation);
        for (dx, dy) in &target_offsets {
            let gx = falling.target_x as f32 + *dx as f32;
            let gy = falling.landing_y as f32 + *dy as f32;
            let world_pos = grid_to_world_pos(gx, gy);

            commands.spawn((
                Sprite {
                    color: ghost_col,
                    custom_size: Some(Vec2::splat(CELL_SIZE - 2.0)),
                    ..default()
                },
                Transform::from_xyz(world_pos.x, world_pos.y, 2.0),
                GhostCellVisual,
            ));
        }

        // 落下中トミノ本体（現在の回転状態）の描画
        let current_offsets = falling.kind.cell_offsets(falling.current_rotation);
        for (dx, dy) in &current_offsets {
            let fx = falling.current_x + *dx as f32;
            let fy = falling.current_y + *dy as f32;
            let world_pos = grid_to_world_pos(fx, fy);

            commands.spawn((
                Sprite {
                    color: player_col,
                    custom_size: Some(Vec2::splat(CELL_SIZE)),
                    ..default()
                },
                Transform::from_xyz(world_pos.x, world_pos.y, 5.0),
                FallingCellVisual,
            ));
        }
    }
}
