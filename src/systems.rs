use crate::ai::{AutoAi, PredictedPlacement};
use crate::board::{GlobalBoard, lane_x_range};
use crate::config::*;
use crate::game::*;
use crate::tromino::TrominoKind;
use bevy::prelude::*;
use bevy_prng::WyRand;
use bevy_rand::prelude::GlobalRng;

/// 新規トミノのスポーンシステム（予測型連携AI対応）
pub fn spawn_tromino_system(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut board: ResMut<GlobalBoard>,
    mut rng_query: Query<&mut WyRand, With<GlobalRng>>,
    mut lane_query: Query<(Entity, &LaneSlot, Option<&mut LaneSpawnCooldown>)>,
    falling_query: Query<&FallingTromino>,
) {
    if board.game_over {
        board.reset();
    }

    let Ok(mut rng) = rng_query.single_mut() else {
        return;
    };

    let mut predicted_others: Vec<PredictedPlacement> = falling_query
        .iter()
        .map(|falling| PredictedPlacement {
            player_id: falling.lane_id,
            kind: falling.kind,
            rotation: falling.target_rotation,
            target_x: falling.target_x,
            landing_y: falling.landing_y,
        })
        .collect();

    let mut air_obstacles: Vec<(i32, i32)> = Vec::new();
    for falling in falling_query.iter() {
        let offsets = falling.kind.cell_offsets(falling.current_rotation);
        for (dx, dy) in &offsets {
            air_obstacles.push((
                falling.current_x.round() as i32 + dx,
                falling.current_y.round() as i32 + dy,
            ));
        }
    }

    for (lane_entity, lane, cooldown_opt) in lane_query.iter_mut() {
        let has_active = falling_query.iter().any(|f| f.lane_id == lane.id);
        if has_active {
            continue;
        }

        if let Some(mut cooldown) = cooldown_opt {
            cooldown.timer.tick(time.delta());
            if !cooldown.timer.is_finished() {
                continue;
            }
            commands.entity(lane_entity).remove::<LaneSpawnCooldown>();
        }

        let kind = TrominoKind::random_from_rng(&mut rng);

        let best_move =
            match AutoAi::find_best_move(&board, lane.id, &kind, &predicted_others, &air_obstacles)
            {
                Some(m) => m,
                None => {
                    commands.entity(lane_entity).insert(LaneSpawnCooldown {
                        timer: Timer::from_seconds(0.2, TimerMode::Once),
                    });
                    continue;
                }
            };

        predicted_others.push(PredictedPlacement {
            player_id: lane.id,
            kind,
            rotation: best_move.rotation,
            target_x: best_move.target_x,
            landing_y: best_move.landing_y,
        });

        let start_y = (LANE_HEIGHT - 1) as f32;
        let (lane_min_x, _) = lane_x_range(lane.id);
        let start_x = lane_min_x as f32;

        // スポーン時は投入口内（幅3）に安全に収まる初期回転（縦向きStraight等）から開始
        // Straight: rot=1 (縦向き 1x3), Corner: rot=0
        let initial_rotation = match kind {
            TrominoKind::Straight => 1,
            TrominoKind::Corner => 0,
        };

        commands.spawn(FallingTromino {
            lane_id: lane.id,
            kind,
            current_rotation: initial_rotation,
            target_rotation: best_move.rotation,
            current_x: start_x,
            current_y: start_y,
            target_x: best_move.target_x,
            landing_y: best_move.landing_y,
            fall_timer: Timer::from_seconds(settings.fall_interval, TimerMode::Repeating),
            lock_timer: Timer::from_seconds(0.06, TimerMode::Once),
            rotate_timer: Timer::from_seconds(0.08, TimerMode::Repeating),
        });
    }
}

/// 指定した (x, y, rotation) においてトミノが盤面（壁・固定ブロック）および他トミノと衝突するか判定
fn check_position_collision(
    board: &GlobalBoard,
    kind: &TrominoKind,
    rotation: usize,
    x: i32,
    y: i32,
    this_entity: Entity,
    current_falling: &[(Entity, Vec<(i32, i32)>)],
) -> bool {
    let offsets = kind.cell_offsets(rotation);

    for (dx, dy) in &offsets {
        let cx = x + dx;
        let cy = y + dy;

        // 1. 盤面の壁・固定ブロックとの衝突
        if board.is_occupied(cx, cy) {
            return true;
        }

        // 2. 落下中の他のトミノとの衝突
        for (other_entity, other_cells) in current_falling {
            if *other_entity == this_entity {
                continue;
            }
            if other_cells.contains(&(cx, cy)) {
                return true;
            }
        }
    }

    false
}

/// 回転衝突判定＆壁キック（Wall Kick）処理
/// 回転先が塞がれている場合、キック候補 (dx, dy) を試して安全な位置へシフト。
/// どこにもキックできない場合は None（回転失敗）を返す
fn try_rotate_with_kick(
    board: &GlobalBoard,
    kind: &TrominoKind,
    from_rot: usize,
    to_rot: usize,
    cur_x: i32,
    cur_y: i32,
    this_entity: Entity,
    current_falling: &[(Entity, Vec<(i32, i32)>)],
) -> Option<(i32, i32)> {
    if from_rot == to_rot {
        return Some((cur_x, cur_y));
    }

    // キック候補オフセット: (0, 0) その場 -> (-1, 0) 左 -> (+1, 0) 右 -> (0, +1) 上 -> (-1, +1) -> (+1, +1)
    let kick_offsets = [(0, 0), (-1, 0), (1, 0), (0, 1), (-1, 1), (1, 1), (0, -1)];

    for (kdx, kdy) in kick_offsets {
        let test_x = cur_x + kdx;
        let test_y = cur_y + kdy;

        let collides = check_position_collision(
            board,
            kind,
            to_rot,
            test_x,
            test_y,
            this_entity,
            current_falling,
        );

        if !collides {
            return Some((test_x, test_y));
        }
    }

    None
}

/// 落下・移動処理システム（移動・回転の完全衝突判定付き）
pub fn falling_tromino_system(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut board: ResMut<GlobalBoard>,
    mut falling_query: Query<(Entity, &mut FallingTromino)>,
    lane_query: Query<(Entity, &LaneSlot)>,
) {
    let dt = time.delta_secs();

    // 各落下中トミノの現在の占有セルリスト（整数座標）
    let current_falling: Vec<(Entity, Vec<(i32, i32)>)> = falling_query
        .iter()
        .map(|(entity, falling)| {
            let offsets = falling.kind.cell_offsets(falling.current_rotation);
            let cells = offsets
                .iter()
                .map(|(dx, dy)| {
                    (
                        falling.current_x.round() as i32 + dx,
                        falling.current_y.round() as i32 + dy,
                    )
                })
                .collect();
            (entity, cells)
        })
        .collect();

    for (tromino_entity, mut falling) in falling_query.iter_mut() {
        let current_int_y = falling.current_y.round() as i32;
        let current_int_x = falling.current_x.round() as i32;

        // 1. 回転処理（回転衝突判定および壁キック）
        if falling.current_rotation != falling.target_rotation {
            falling.rotate_timer.tick(time.delta());
            if falling.rotate_timer.just_finished() {
                // 次の回転状態へ1ステップ回転
                let max_rot = falling.kind.rotation_count();
                let next_rot = (falling.current_rotation + 1) % max_rot;

                // 回転衝突判定＆キックチェック
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
        let can_move_horizontal = falling.current_y < SPAWN_WALL_MIN_Y as f32;
        if can_move_horizontal {
            let dx = falling.target_x as f32 - falling.current_x;
            if dx.abs() > 0.01 {
                let step = 18.0 * dt;
                let next_x = if dx.abs() <= step {
                    falling.target_x as f32
                } else {
                    falling.current_x + step * dx.signum()
                };

                let next_int_x = next_x.round() as i32;
                let collides = check_position_collision(
                    &board,
                    &falling.kind,
                    falling.current_rotation,
                    next_int_x,
                    current_int_y,
                    tromino_entity,
                    &current_falling,
                );

                if !collides {
                    falling.current_x = next_x;
                }
            }
        }

        let updated_int_x = falling.current_x.round() as i32;

        let mut bottom_blocked = false;

        // 3. 下降処理（直下が塞がっている場合は即座に接地とみなす）
        falling.fall_timer.tick(time.delta());
        if falling.fall_timer.just_finished() {
            let next_int_y = current_int_y - 1;

            let bottom_hit_board = {
                let offsets = falling.kind.cell_offsets(falling.current_rotation);
                offsets.iter().any(|(dx, dy)| {
                    let cx = updated_int_x + dx;
                    let cy = next_int_y + dy;
                    board.is_occupied(cx, cy)
                })
            };

            if bottom_hit_board {
                bottom_blocked = true;
                falling.landing_y = current_int_y;
                falling.current_y = current_int_y as f32;
            } else {
                let bottom_hit_other = {
                    let offsets = falling.kind.cell_offsets(falling.current_rotation);
                    current_falling.iter().any(|(other_ent, other_cells)| {
                        if *other_ent == tromino_entity {
                            return false;
                        }
                        offsets.iter().any(|(dx, dy)| {
                            let cx = updated_int_x + dx;
                            let cy = next_int_y + dy;
                            other_cells.contains(&(cx, cy))
                        })
                    })
                };

                if bottom_hit_other {
                    bottom_blocked = true;
                    falling.landing_y = current_int_y;
                    falling.current_y = current_int_y as f32;
                } else if falling.current_y > falling.landing_y as f32 {
                    falling.current_y = (falling.current_y - 1.0).max(falling.landing_y as f32);
                }
            }
        }

        // 4. 着地点到達 & 固定処理
        let arrived_landing = falling.current_y <= falling.landing_y as f32 + 0.05;
        let arrived_x = (falling.current_x - falling.target_x as f32).abs() < 0.15;
        let arrived_rotation = falling.current_rotation == falling.target_rotation;

        // 目標到達、または真下が塞がってこれ以上落ちられない場合はロック進行
        if (arrived_landing && arrived_x && arrived_rotation) || bottom_blocked {
            falling.lock_timer.tick(time.delta());
            if falling.lock_timer.is_finished() {
                let lock_x = falling.current_x.round() as i32;
                let lock_y = falling.current_y.round() as i32;

                let safe_to_lock =
                    board.can_place(&falling.kind, falling.current_rotation, lock_x, lock_y);

                if safe_to_lock {
                    let locked = board.lock_tromino(
                        falling.lane_id,
                        &falling.kind,
                        falling.current_rotation,
                        lock_x,
                        lock_y,
                    );

                    if locked {
                        board.clear_full_lines();
                    }
                }

                for (lane_entity, lane) in lane_query.iter() {
                    if lane.id == falling.lane_id {
                        // ランダムな揺らぎ（0.05〜0.25s）をつけて同調スポーンによる空中衝突を分散
                        let delay = settings.spawn_delay + (falling.lane_id as f32 * 0.04);
                        commands.entity(lane_entity).insert(LaneSpawnCooldown {
                            timer: Timer::from_seconds(delay, TimerMode::Once),
                        });
                        break;
                    }
                }

                commands.entity(tromino_entity).despawn();
            }
        }
    }
}
