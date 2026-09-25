use crate::ai::{AutoAi, PredictedPlacement};
use crate::board::{GlobalBoard, lane_x_range};
use crate::config::*;
use crate::game::*;
use crate::tromino::TrominoKind;
use bevy::prelude::*;
use bevy_prng::WyRand;
use bevy_rand::prelude::GlobalRng;

/// 各レーンのシグナル更新システム（人間的な低解像度情報：穴の位置・状態・各自の自己申告ペース・意図）
pub fn update_lane_signals_system(
    board: Res<GlobalBoard>,
    mut signal_board: ResMut<LaneSignalBoard>,
    falling_query: Query<&FallingTromino>,
) {
    for lane_id in 0..LANE_COUNT {
        // 1. 空白（穴）情報の抽出
        let (hole_x, hole_status) = match board.find_lane_deepest_hole(lane_id) {
            Some((hx, _hy, has_roof)) => {
                if has_roof {
                    (Some(hx), HoleStatus::WaitingForClearance)
                } else {
                    (Some(hx), HoleStatus::ReadyForFill)
                }
            }
            None => (None, HoleStatus::None),
        };

        // 2. 深さ2以上の縦穴（I字で埋めたい溝）の検出
        let vertical_well = board
            .find_lane_vertical_wells(lane_id)
            .into_iter()
            .max_by_key(|&(_, _, depth)| depth);

        // 3. 落下中トミノ（各レーンAI）からの自己申告意図（Intent）および自己決定ペースの抽出
        let falling_opt = falling_query.iter().find(|f| f.lane_id == lane_id);
        let intent_target_x = falling_opt.map(|f| f.target_x);
        let pace = falling_opt.map(|f| f.pace).unwrap_or(LanePace::Normal);

        signal_board.signals[lane_id] = LaneSignal {
            lane_id,
            hole_x,
            hole_status,
            vertical_well,
            intent_target_x,
            pace,
        };
    }
}


/// 新規トミノのスポーンシステム（限定シグナル＆ペース連動対応）
pub fn spawn_tromino_system(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut board: ResMut<GlobalBoard>,
    signals: Res<LaneSignalBoard>,
    mut logger: ResMut<GameLogger>,
    mut rng_query: Query<&mut WyRand, With<GlobalRng>>,
    mut lane_query: Query<(Entity, &LaneSlot, Option<&mut LaneSpawnCooldown>)>,
    falling_query: Query<&FallingTromino>,
) {
    if board.game_over {
        // 全員積み（ゲームオーバー）時はスポーンを停止
        return;
    }

    let Ok(mut rng) = rng_query.single_mut() else {
        return;
    };

    let base_interval = settings.current_base_fall_interval(board.lines_cleared);

    // 各落下中トミノの残り着地ステップ数（ETA）を算出して着地予測順にソート
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
    let mut predicted_others: Vec<PredictedPlacement> = falling_etas.into_iter().map(|(_, p)| p).collect();

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

        let (lane_min_x, _) = lane_x_range(lane.id);
        let start_x = lane_min_x as f32;
        let start_y = (LANE_HEIGHT - 1) as f32;

        let kind = TrominoKind::random_from_rng(&mut rng);

        // スポーン時の初期姿勢
        let initial_rotation = match kind {
            TrominoKind::Straight => 1,
            TrominoKind::Corner => 0,
        };

        // 投入口直下にブロックが詰まっているかチェック
        let can_enter_spawn = board.can_place(&kind, initial_rotation, start_x as i32, start_y as i32);

        let best_move = if can_enter_spawn {
            AutoAi::find_best_move(&board, lane.id, &kind, &predicted_others, &air_obstacles, &signals)
        } else {
            None
        };

        match best_move {
            Some(m) => {
                // スポーン成功 -> 当該レーンの積み状態は解消（回復）
                board.lane_stuck[lane.id] = false;

                let spawn_msg = format!(
                    "[AI Lane {}] Spawning {:?} -> Target (x:{}, y:{}, rot:{}, pace:{:?}) | Breakdown: {}",
                    lane.id + 1,
                    kind,
                    m.target_x,
                    m.landing_y,
                    m.rotation,
                    m.pace,
                    m.breakdown,
                );
                debug!("{}", spawn_msg);
                logger.log(&spawn_msg);

                predicted_others.push(PredictedPlacement {
                    player_id: lane.id,
                    kind,
                    rotation: m.rotation,
                    target_x: m.target_x,
                    landing_y: m.landing_y,
                });

                let lane_pace = m.pace;
                let fall_interval = match lane_pace {
                    LanePace::SoftDrop => base_interval * settings.soft_drop_multiplier,
                    LanePace::Normal => base_interval,
                };

                commands.spawn(FallingTromino {
                    lane_id: lane.id,
                    kind,
                    current_rotation: initial_rotation,
                    target_rotation: m.rotation,
                    current_x: start_x,
                    current_y: start_y,
                    target_x: m.target_x,
                    landing_y: m.landing_y,
                    fall_timer: Timer::from_seconds(fall_interval, TimerMode::Repeating),
                    lock_timer: Timer::from_seconds(LOCK_DELAY, TimerMode::Once),
                    rotate_timer: Timer::from_seconds(0.08, TimerMode::Repeating),
                    is_on_ground: false,
                    lock_resets_left: MAX_LOCK_RESETS,
                    pace: lane_pace,
                    waypoints: m.waypoints,
                    planned_board_version: board.board_version,
                    replan_timer: Timer::from_seconds(0.60, TimerMode::Repeating),
                });
            }
            None => {
                // 次の形が出せない（一時的な積み）状態
                let heights = board.column_heights();
                let max_h = heights.iter().max().copied().unwrap_or(0);
                let holes = board.count_holes();
                let (lane_min_x, lane_max_x) = lane_x_range(lane.id);
                let lane_max_h = (lane_min_x..=lane_max_x).map(|x| heights[x]).max().unwrap_or(0);

                if !can_enter_spawn {
                    board.lane_stuck[lane.id] = true;
                    let stuck_msg = format!(
                        "[LANE STUCK] Lane {} entrance blocked! (LaneMaxH: {}, GlobalMaxH: {}, Holes: {})",
                        lane.id + 1,
                        lane_max_h,
                        max_h,
                        holes
                    );
                    warn!("{}", stuck_msg);
                    logger.log(&stuck_msg);

                    if board.lane_stuck.iter().all(|&stuck| stuck) {
                        board.game_over = true;
                        let game_over_msg = format!(
                            "[GAME OVER] All lanes stuck! Final Lines: {}, Score: {}, Holes: {}, MaxH: {}",
                            board.lines_cleared,
                            board.score,
                            holes,
                            max_h
                        );
                        error!("{}", game_over_msg);
                        logger.log(&game_over_msg);
                    }
                } else {
                    let no_path_msg = format!(
                        "[AI Lane {}] No valid path/placement found for {:?} (LaneMaxH: {}, Holes: {})",
                        lane.id + 1,
                        kind,
                        lane_max_h,
                        holes
                    );
                    debug!("{}", no_path_msg);
                    logger.log(&no_path_msg);
                }

                commands.entity(lane_entity).insert(LaneSpawnCooldown {
                    timer: Timer::from_seconds(0.25, TimerMode::Once),
                });
            }
        }
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
    signals: Res<LaneSignalBoard>,
    mut logger: ResMut<GameLogger>,
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

    // 空中の他トミノのセル（動的障害物）
    let all_air_obstacles: Vec<(i32, i32)> = current_falling
        .iter()
        .flat_map(|(_, cells)| cells.clone())
        .collect();

    // 他トミノの着地予測リスト（落下順/ETA順にソート）
    let base_interval = settings.current_base_fall_interval(board.lines_cleared);
    let mut other_etas: Vec<(f32, PredictedPlacement)> = falling_query
        .iter()
        .map(|(_ent, f)| {
            let dy = (f.current_y - f.landing_y as f32).max(0.0);
            let interval = match f.pace {
                LanePace::SoftDrop => base_interval * settings.soft_drop_multiplier,
                LanePace::Normal => base_interval,
            };
            (
                dy * interval,
                PredictedPlacement {
                    player_id: f.lane_id,
                    kind: f.kind,
                    rotation: f.target_rotation,
                    target_x: f.target_x,
                    landing_y: f.landing_y,
                },
            )
        })
        .collect();
    other_etas.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let all_predicted_others: Vec<PredictedPlacement> = other_etas.into_iter().map(|(_, p)| p).collect();

    for (tromino_entity, mut falling) in falling_query.iter_mut() {
        let current_int_y = falling.current_y.round() as i32;
        let current_int_x = falling.current_x.round() as i32;

        // 0. 落下目的地点の再計算（Dynamic Re-planning）
        // - 盤面変化（ライン消去や他者着地で board_version が更新された）
        // - 現在の目標着地点がすでに衝突・占有されてしまい安全に着地できない（予想外の事態）
        // - 一定周期（replan_timer）での状況チェック
        falling.replan_timer.tick(time.delta());

        let target_invalidated = !board.can_place(
            &falling.kind,
            falling.target_rotation,
            falling.target_x,
            falling.landing_y,
        );

        let board_changed = falling.planned_board_version != board.board_version;
        let should_replan = !falling.is_on_ground
            && (target_invalidated || board_changed || falling.replan_timer.just_finished());

        if should_replan {
            let this_others: Vec<PredictedPlacement> = all_predicted_others
                .iter()
                .filter(|p| p.player_id != falling.lane_id)
                .cloned()
                .collect();

            if let Some(re_eval) = AutoAi::find_best_move_from_position(
                &board,
                falling.lane_id,
                &falling.kind,
                current_int_x,
                current_int_y,
                falling.current_rotation,
                Some((falling.target_x, falling.target_rotation)),
                &this_others,
                &all_air_obstacles,
                &signals,
            ) {
                if falling.target_x != re_eval.target_x
                    || falling.landing_y != re_eval.landing_y
                    || falling.target_rotation != re_eval.rotation
                {
                    let replan_msg = format!(
                        "[AI Lane {}] Replan shift: (x:{}, y:{}, rot:{}) -> (x:{}, y:{}, rot:{}) | Breakdown: {}",
                        falling.lane_id + 1,
                        falling.target_x,
                        falling.landing_y,
                        falling.target_rotation,
                        re_eval.target_x,
                        re_eval.landing_y,
                        re_eval.rotation,
                        re_eval.breakdown,
                    );
                    trace!("{}", replan_msg);
                    logger.log(&replan_msg);
                }

                // 目標地点および姿勢、ウェイポイントを最新状況に更新
                falling.target_x = re_eval.target_x;
                falling.landing_y = re_eval.landing_y;
                falling.target_rotation = re_eval.rotation;
                falling.waypoints = re_eval.waypoints;
                falling.pace = re_eval.pace;
                falling.planned_board_version = board.board_version;

                // 落下ペースに応じたインターバル再適用
                let fall_interval = match falling.pace {
                    LanePace::SoftDrop => base_interval * settings.soft_drop_multiplier,
                    LanePace::Normal => base_interval,
                };
                falling.fall_timer.set_duration(std::time::Duration::from_secs_f32(fall_interval));
            }
        }

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
        let (lane_min_x, lane_max_x) = lane_x_range(falling.lane_id);
        let offsets = falling.kind.cell_offsets(falling.current_rotation);
        let max_dy = offsets.iter().map(|(_, dy)| *dy).max().unwrap_or(0);
        let min_dx = offsets.iter().map(|(dx, _)| *dx).min().unwrap_or(0);
        let max_dx = offsets.iter().map(|(dx, _)| *dx).max().unwrap_or(0);

        // トミノの全パーツが完全に仕切り壁より下にあるか判定
        let fully_below_spawn_wall = (falling.current_y.round() as i32 + max_dy) < SPAWN_WALL_MIN_Y as i32;

        // 2. ウェイポイントの更新と横移動処理
        // ウェイポイントがある場合、現在の高度・位置に合わせて到達判定と次の目標決定を行う
        while !falling.waypoints.is_empty() {
            let (wp_x, wp_y) = falling.waypoints[0];
            // ウェイポイントの X に到達しており、かつ高度も通過または到達していれば消費
            if (falling.current_x - wp_x as f32).abs() <= 0.05
                && (current_int_y <= wp_y || (falling.waypoints.len() > 1 && falling.waypoints[1].1 < wp_y))
            {
                falling.waypoints.remove(0);
            } else if current_int_y < wp_y && (falling.current_x - wp_x as f32).abs() > 0.05 {
                // すでにそのウェイポイントの高度より下回ってしまっている場合はスキップして次へ
                falling.waypoints.remove(0);
            } else {
                break;
            }
        }

        let curr_target_x = if let Some(&(wp_x, wp_y)) = falling.waypoints.first() {
            // もしウェイポイントの高度よりまだ高い段にいる場合:
            // 途中に障害物（屋根）があるかもしれないので、そのウェイポイントの高度 wp_y 付近まで降りるのを優先。
            // ただし wp_x への横移動が安全（衝突なし）なら早期に横移動してもよい。
            if current_int_y > wp_y {
                // 現在の高さで wp_x へ向かうと衝突するか事前チェック
                let test_dir = (wp_x as f32 - falling.current_x).signum();
                let test_next_int_x = (falling.current_x + test_dir).round() as i32;
                let test_block_min_x = test_next_int_x + min_dx;
                let test_block_max_x = test_next_int_x + max_dx;
                let test_inside = test_block_min_x >= lane_min_x as i32 && test_block_max_x <= lane_max_x as i32;
                if (test_inside || fully_below_spawn_wall) && check_position_collision(
                    &board,
                    &falling.kind,
                    falling.current_rotation,
                    test_next_int_x,
                    current_int_y,
                    tromino_entity,
                    &current_falling,
                ) {
                    // 頭上や側面に障害物があるため、wp_y まで下降するまでは横移動を保留
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

            // 移動方向へ進んだ際に触れる直近の整数座標を判定
            let check_int_x = if dir > 0.0 {
                next_x.ceil() as i32
            } else {
                next_x.floor() as i32
            };

            let block_min_x = check_int_x + min_dx;
            let block_max_x = check_int_x + max_dx;
            let is_inside_slot = block_min_x >= lane_min_x as i32 && block_max_x <= lane_max_x as i32;

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
                    // 接地中に横スライド移動が成功した場合、ロックタイマーをリセット（スライディング猶予時間）
                    if falling.is_on_ground && falling.lock_resets_left > 0 {
                        falling.lock_timer.reset();
                        falling.lock_resets_left -= 1;
                    }
                } else {
                    // 障害物にぶつかる場合は直前の安全な整数座標にピタッと寄せる
                    let safe_stop_x = current_int_x as f32;
                    if (falling.current_x - safe_stop_x).abs() <= step {
                        falling.current_x = safe_stop_x;
                    }
                }
            }
        }

        let updated_int_x = falling.current_x.round() as i32;

        // 3. 接地判定（床またはボード上の固定ブロックに直下で乗っているか）
        let offsets = falling.kind.cell_offsets(falling.current_rotation);
        let is_on_ground = offsets.iter().any(|(dx, dy)| {
            let cx = updated_int_x + dx;
            let cy = current_int_y + dy - 1;
            board.is_occupied(cx, cy)
        });

        // 接地状態の変化を追跡
        if is_on_ground && !falling.is_on_ground {
            // 空中から接地した瞬間: ロックタイマー開始
            falling.is_on_ground = true;
            falling.lock_timer.reset();
        } else if !is_on_ground {
            // 横移動や落下によって再び浮いた場合: 接地解除
            falling.is_on_ground = false;
        }

        // 4. 下降処理
        falling.fall_timer.tick(time.delta());
        if falling.fall_timer.just_finished() {
            if !is_on_ground {
                let next_int_y = current_int_y - 1;
                // 他の落下中トミノが直下にあるか判定
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

                // 他の落下トミノと重ならない場合のみ1段降下
                if !bottom_hit_other {
                    falling.current_y = (current_int_y - 1) as f32;
                }
            } else {
                falling.current_y = current_int_y as f32;
            }
        }

        // 5. 固定処理（床または固定ブロックに接地している場合のみロックタイマーを進行）
        if is_on_ground {
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
                        let lines = board.clear_full_lines();
                        if lines > 0 {
                            let clear_msg = format!(
                                "[LINE CLEAR] {} lines cleared by P{}! Total Lines: {}, Score: {}",
                                lines,
                                falling.lane_id + 1,
                                board.lines_cleared,
                                board.score
                            );
                            info!("{}", clear_msg);
                            logger.log(&clear_msg);
                        } else {
                            let lock_msg = format!(
                                "[LOCKED] P{} {:?} locked at (x:{}, y:{}, rot:{})",
                                falling.lane_id + 1,
                                falling.kind,
                                lock_x,
                                lock_y,
                                falling.current_rotation
                            );
                            debug!("{}", lock_msg);
                            logger.log(&lock_msg);
                        }
                    } else {
                        let heights = board.column_heights();
                        let max_h = heights.iter().max().copied().unwrap_or(0);
                        let overflow_msg = format!(
                            "[TOP OVERFLOW] P{} locked above ceiling! (GlobalMaxH: {}, Lines: {})",
                            falling.lane_id + 1,
                            max_h,
                            board.lines_cleared
                        );
                        warn!("{}", overflow_msg);
                        logger.log(&overflow_msg);
                    }
                } else {
                    let fail_msg = format!(
                        "[LOCK FAILED] P{} {:?} cannot be placed at (x:{}, y:{}, rot:{})",
                        falling.lane_id + 1,
                        falling.kind,
                        lock_x,
                        lock_y,
                        falling.current_rotation
                    );
                    warn!("{}", fail_msg);
                    logger.log(&fail_msg);
                }

                for (lane_entity, lane) in lane_query.iter() {
                    if lane.id == falling.lane_id {
                        // ソフトドロップ（急ぎ）のレーンはスポーンクールダウンも短縮して次弾を急ぐ
                        let pace_multiplier = match falling.pace {
                            LanePace::SoftDrop => 0.65,
                            LanePace::Normal => 1.0,
                        };
                        let delay = (settings.spawn_delay + (falling.lane_id as f32 * 0.04)) * pace_multiplier;

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
