pub mod collision;
pub mod lock;
pub mod replan;

pub use collision::{check_position_collision, try_rotate_with_kick};
pub use lock::handle_lock_process;
pub use replan::update_replan_if_needed;

use crate::board::{GlobalBoard, lane_x_range};
use crate::config::{GameSettings, SPAWN_WALL_MIN_Y};
use crate::game::{FallingTromino, GameLogger, LaneSignalBoard, LaneSlot};
use bevy::prelude::*;

/// 落下・移動処理システム（移動・回転の完全衝突判定付き）
#[allow(clippy::too_many_arguments)]
pub fn falling_tromino_system(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut board: ResMut<GlobalBoard>,
    signals: Res<LaneSignalBoard>,
    mut logger: ResMut<GameLogger>,
    mut falling_query: Query<(Entity, &mut FallingTromino)>,
    lane_query: Query<(Entity, &LaneSlot)>,
) {
    let dt = time.delta_secs();

    // 各落下中トミノの現在の占有セルリスト（整数座標）
    let current_falling: Vec<(Entity, Vec<(i32, i32)>)> = falling_query
        .iter()
        .map(|(entity, falling)| (entity, falling.occupied_cells()))
        .collect();

    // 空中の他トミノのセル（動的障害物）
    let all_air_obstacles: Vec<(i32, i32)> = current_falling
        .iter()
        .flat_map(|(_, cells)| cells.iter().copied())
        .collect();

    // 他トミノの着地予測リスト（落下順/ETA順にソート）
    let all_predicted_others = crate::systems::spawn::collect_sorted_predictions(
        falling_query.iter().map(|(_, f)| f),
        &settings,
        board.lines_cleared,
    );

    let base_interval = settings.current_base_fall_interval(board.lines_cleared);

    for (tromino_entity, mut falling) in falling_query.iter_mut() {
        let current_int_y = falling.current_y.round() as i32;
        let current_int_x = falling.current_x.round() as i32;

        // 0. 落下目的地点の再計算（Dynamic Re-planning）
        update_replan_if_needed(
            &mut falling,
            &board,
            &all_predicted_others,
            &all_air_obstacles,
            &signals,
            &settings,
            base_interval,
            &mut logger,
            current_int_x,
            current_int_y,
            time.delta(),
        );

        // 1. 回転処理（回転衝突判定および壁キック）
        if falling.current_rotation != falling.target_rotation {
            falling.rotate_timer.tick(time.delta());
            if falling.rotate_timer.just_finished() {
                let max_rot = falling.kind.rotation_count();
                let next_rot = (falling.current_rotation + 1) % max_rot;

                if let Some((safe_x, safe_y)) = try_rotate_with_kick(
                    &board,
                    &falling.kind,
                    falling.current_rotation,
                    next_rot,
                    current_int_x,
                    current_int_y,
                    tromino_entity,
                    &current_falling,
                ) {
                    falling.current_rotation = next_rot;
                    falling.current_x = safe_x as f32;
                    falling.current_y = safe_y as f32;
                }
            }
        }

        // 2. 横移動処理（壁・固定ブロック・他落下トミノとの移動先衝突判定）
        let (lane_min_x, lane_max_x) = lane_x_range(falling.lane_id);
        let offsets = falling.kind.cell_offsets(falling.current_rotation);
        let max_dy = offsets.iter().map(|(_, dy)| *dy).max().unwrap_or(0);
        let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap_or(0);
        let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap_or(0);

        let fully_below_spawn_wall =
            (falling.current_y.round() as i32 + max_dy) < SPAWN_WALL_MIN_Y as i32;

        while !falling.waypoints.is_empty() {
            let (wp_x, wp_y) = falling.waypoints[0];
            let reached_x = (falling.current_x - wp_x as f32).abs() <= 0.05;
            if (reached_x
                && (current_int_y <= wp_y
                    || (falling.waypoints.len() > 1 && falling.waypoints[1].1 < wp_y)))
                || (current_int_y < wp_y && !reached_x)
            {
                falling.waypoints.remove(0);
            } else {
                break;
            }
        }

        let curr_target_x = if let Some(&(wp_x, wp_y)) = falling.waypoints.first() {
            if current_int_y > wp_y {
                let test_dir = (wp_x as f32 - falling.current_x).signum();
                let test_next_int_x = (falling.current_x + test_dir).round() as i32;
                let test_block_min_x = test_next_int_x + min_dx;
                let test_block_max_x = test_next_int_x + max_dx;
                let test_inside =
                    test_block_min_x >= lane_min_x as i32 && test_block_max_x <= lane_max_x as i32;
                if (test_inside || fully_below_spawn_wall)
                    && check_position_collision(
                        &board,
                        &falling.kind,
                        falling.current_rotation,
                        test_next_int_x,
                        current_int_y,
                        tromino_entity,
                        &current_falling,
                    )
                {
                    current_int_x
                } else {
                    wp_x
                }
            } else {
                wp_x
            }
        } else {
            falling.target_x
        };

        let dx = curr_target_x as f32 - falling.current_x;
        if dx.abs() > 0.01 {
            let step = 18.0 * dt;
            let dir = dx.signum();
            let next_x = if dx.abs() <= step {
                curr_target_x as f32
            } else {
                falling.current_x + step * dir
            };

            let check_int_x = if dir > 0.0 {
                next_x.ceil() as i32
            } else {
                next_x.floor() as i32
            };

            let block_min_x = check_int_x + min_dx;
            let block_max_x = check_int_x + max_dx;
            let is_inside_slot =
                block_min_x >= lane_min_x as i32 && block_max_x <= lane_max_x as i32;

            if is_inside_slot || fully_below_spawn_wall {
                let collides = check_position_collision(
                    &board,
                    &falling.kind,
                    falling.current_rotation,
                    check_int_x,
                    current_int_y,
                    tromino_entity,
                    &current_falling,
                );

                if !collides {
                    falling.current_x = next_x;
                    if falling.is_on_ground && falling.lock_resets_left > 0 {
                        falling.lock_timer.reset();
                        falling.lock_resets_left -= 1;
                    }
                } else {
                    let safe_stop_x = current_int_x as f32;
                    if (falling.current_x - safe_stop_x).abs() <= step {
                        falling.current_x = safe_stop_x;
                    }
                }
            }
        }

        let updated_int_x = falling.current_x.round() as i32;

        // 3. 接地判定
        let offsets = falling.kind.cell_offsets(falling.current_rotation);
        let is_on_ground = offsets.iter().any(|(dx, dy)| {
            let cx = updated_int_x + dx;
            let cy = current_int_y + dy - 1;
            board.is_occupied(cx, cy)
        });

        if is_on_ground && !falling.is_on_ground {
            falling.is_on_ground = true;
            falling.lock_timer.reset();
        } else if !is_on_ground {
            falling.is_on_ground = false;
        }

        // 4. 下降処理
        falling.fall_timer.tick(time.delta());
        if falling.fall_timer.just_finished() {
            if !is_on_ground {
                let next_int_y = current_int_y - 1;
                let bottom_hit_other = current_falling.iter().any(|(other_ent, other_cells)| {
                    if *other_ent == tromino_entity {
                        return false;
                    }
                    offsets.iter().any(|(dx, dy)| {
                        let cx = updated_int_x + dx;
                        let cy = next_int_y + dy;
                        other_cells.contains(&(cx, cy))
                    })
                });

                if !bottom_hit_other {
                    falling.current_y = (current_int_y - 1) as f32;
                }
            } else {
                falling.current_y = current_int_y as f32;
            }
        }

        // 5. 固定処理
        if is_on_ground {
            falling.lock_timer.tick(time.delta());
            if falling.lock_timer.is_finished() {
                handle_lock_process(
                    &mut commands,
                    tromino_entity,
                    &falling,
                    &mut board,
                    &mut logger,
                    &settings,
                    &lane_query,
                );
            }
        }
    }
}
