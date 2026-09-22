#[cfg(test)]
mod tests {
    use crate::board::{is_border_column, GlobalBoard};
    use crate::config::{player_color, SPAWN_WALL_MIN_Y, TOTAL_GRID_WIDTH};
    use crate::tromino::TrominoKind;
    use crate::ai::{AutoAi, PredictedPlacement};

    #[test]
    fn test_border_column_and_wall() {
        assert!(is_border_column(3));
        assert!(is_border_column(7));
        assert!(is_border_column(11));
        assert!(!is_border_column(0));
        assert!(!is_border_column(2));
        assert!(!is_border_column(4));

        let board = GlobalBoard::default();
        assert!(board.is_occupied(3, SPAWN_WALL_MIN_Y as i32));
        assert!(board.is_occupied(7, (SPAWN_WALL_MIN_Y + 1) as i32));
        assert!(!board.is_occupied(3, (SPAWN_WALL_MIN_Y - 1) as i32));
        assert!(!board.is_occupied(7, 0));
    }

    #[test]
    fn test_player_colored_lock_and_wide_line_clear() {
        let mut board = GlobalBoard::default();
        let kind = TrominoKind::Straight;

        board.lock_tromino(0, &kind, 0, 0, 0);
        assert_eq!(board.cells[0][0], Some(player_color(0)));
        assert_eq!(board.cells[0][1], Some(player_color(0)));
        assert_eq!(board.cells[0][2], Some(player_color(0)));

        for x in 3..TOTAL_GRID_WIDTH {
            board.cells[0][x] = Some(player_color(1));
        }

        let cleared = board.clear_full_lines();
        assert_eq!(cleared, 1);
        assert_eq!(board.lines_cleared, 1);
    }

    #[test]
    fn test_predictive_cooperative_ai_completes_joint_line() {
        let mut board = GlobalBoard::default();

        for x in 3..TOTAL_GRID_WIDTH {
            board.cells[0][x] = Some(player_color(2));
        }

        let kind = TrominoKind::Straight;
        let predicted_others = vec![PredictedPlacement {
            player_id: 1,
            kind: TrominoKind::Straight,
            rotation: 0,
            target_x: 4,
            landing_y: 5,
        }];

        let signals = crate::game::LaneSignalBoard::default();
        let best_move = AutoAi::find_best_move(&board, 0, &kind, &predicted_others, &[], &signals)
            .expect("Move found");

        assert_eq!(best_move.rotation, 0);
        assert_eq!(best_move.target_x, 0);
        assert_eq!(best_move.landing_y, 0);
    }

    #[test]
    fn test_ai_places_on_border_column() {
        let mut board = GlobalBoard::default();
        board.cells[0][0] = Some(player_color(0));
        board.cells[0][1] = Some(player_color(0));

        let kind = TrominoKind::Straight;
        let signals = crate::game::LaneSignalBoard::default();
        let best_move = AutoAi::find_best_move(&board, 0, &kind, &[], &[], &signals).expect("Move found");

        let offsets = kind.cell_offsets(best_move.rotation);
        let uses_border = offsets
            .iter()
            .any(|(dx, _)| is_border_column((best_move.target_x + dx) as usize));

        assert!(uses_border, "AI should utilize border columns when appropriate");
    }


    #[test]
    fn test_movement_destination_collision_prevention() {
        let mut board = GlobalBoard::default();
        let kind = TrominoKind::Straight;

        board.cells[5][2] = Some(player_color(3));

        assert!(!board.can_place(&kind, 0, 1, 5));
        assert!(board.can_place(&kind, 0, 3, 5));
    }

    #[test]
    fn test_rotation_collision_check() {
        let board = GlobalBoard::default();
        let kind = TrominoKind::Straight;

        // 右端 x = 30 に縦向き (1x3: rot=1, x=30, y=0..2) がある場合
        assert!(board.can_place(&kind, 1, 30, 0));

        // 右端で横向き (3x1: rot=0, x=30,31,32) に回転しようとすると、
        // x=31, 32 は盤面外 (TOTAL_GRID_WIDTH=31 なので x >= 31 は外壁)
        // したがって回転先は衝突判定で弾かれる
        assert!(!board.can_place(&kind, 0, 30, 0));

        // 左に2マスシフト (x=28) すれば、横向き (28, 29, 30) で収まるため配置可能
        assert!(board.can_place(&kind, 0, 28, 0));
    }

    #[test]
    fn test_tuck_in_pathfinding_under_overhang() {
        let mut board = GlobalBoard::default();
        // (x=1, y=2) にブロックを置いて直下落下を遮断 (overhang)
        // 下段 (x=1, y=0) は空洞。
        board.cells[2][1] = Some(player_color(0));

        let kind = TrominoKind::Straight;
        // x=1 へ垂直落下しようとすると y=3 で止まるはず
        let direct_y = AutoAi::simulate_drop_with_tuck(&board, 0, &kind, 1, 1, &[], &[]);
        assert!(direct_y.is_some());
    }

    #[test]
    fn test_hole_detection_and_progress_pacing() {
        let mut board = GlobalBoard::default();
        // Lane 0 (x=0,1,2) に穴を作成: x=0 の y=2 にブロック、y=0,1 は空洞
        board.cells[2][0] = Some(player_color(0));

        let hole = board.find_lane_deepest_hole(0);
        assert!(hole.is_some());
        let (hx, hy, has_roof) = hole.unwrap();
        assert_eq!(hx, 0);
        assert!(has_roof, "Should detect roof over hole");
        assert!(hy < 2);

        // 充足率チェック: 空のボードでは 0.0
        let fresh_board = GlobalBoard::default();
        assert_eq!(fresh_board.lane_fill_ratio_at_target_line(0), 0.0);

        // board には y=2 に1個あるので、y=2 が最も埋まっている行になり 1/3 (0.333...) となる
        let ratio = board.lane_fill_ratio_at_target_line(0);
        assert!((ratio - 1.0 / 3.0).abs() < 0.01);

        // 自レーンAIによるペース自己決定のテスト: 空白・遅れがあるため SoftDrop（下キー入力）を自己決定するはず
        let kind = TrominoKind::Straight;
        let signals = crate::game::LaneSignalBoard::default();
        let best_move = AutoAi::find_best_move(&board, 0, &kind, &[], &[], &signals).expect("Move found");
        assert_eq!(best_move.pace, crate::game::LanePace::SoftDrop);
    }

    #[test]
    fn test_lines_cleared_speedup_and_soft_drop() {
        let settings = crate::config::GameSettings::default();
        let initial_speed = settings.current_base_fall_interval(0);
        let speed_after_20_lines = settings.current_base_fall_interval(20);
        let speed_after_50_lines = settings.current_base_fall_interval(50);
        let speed_after_100_lines = settings.current_base_fall_interval(100);

        assert_eq!(initial_speed, 0.35, "Initial fall interval should be 0.35s");
        assert!(speed_after_20_lines < initial_speed, "Speed should accelerate as lines are cleared");
        assert!(speed_after_50_lines < speed_after_20_lines);
        assert!(speed_after_100_lines <= speed_after_50_lines);
        // 100消し程度で最高難度（min_fall_interval 0.04s）に到達
        assert_eq!(speed_after_100_lines, settings.min_fall_interval, "Should reach maximum difficulty around 100 lines cleared");

        // SoftDrop 時はさらに倍率がかかり高速
        let soft_drop_interval = speed_after_20_lines * settings.soft_drop_multiplier;
        assert!(soft_drop_interval < speed_after_20_lines);
    }

    #[test]
    fn test_ai_handles_high_placement_without_panic() {
        // フィールド上部ギリギリ (y >= LANE_HEIGHT) に積み上がった状態でのAI探索でパニックしないことを検証
        let mut board = GlobalBoard::default();
        for y in 0..(crate::config::LANE_HEIGHT - 2) {
            board.cells[y][0] = Some(player_color(0));
            board.cells[y][1] = Some(player_color(0));
            board.cells[y][2] = Some(player_color(0));
        }

        let kind = TrominoKind::Straight;
        let signals = crate::game::LaneSignalBoard::default();
        let _ = AutoAi::find_best_move(&board, 0, &kind, &[], &[], &signals);
    }

    #[test]
    fn test_corner_ground_slide_into_alcove() {
        // 横の凹みになっている場所への接地時横移動（L字 / Corner）テスト
        let mut board = GlobalBoard::default();
        // Lane 0 (x=0,1,2)
        // x=0, y=1 にブロックを置き、x=0, y=0 は横の凹み（空洞）
        // 床 (y=0) は空いているので、x=0, y=0 に Corner (rot=3: (1,1), (0,0), (1,0)) を配置する。
        // rot=3 は (0,1) が空いているため、(0,0) に収まることができる！
        // しかし直上 (x=0) から垂直落下しようとすると (1,1) のブロック部分が y=1 付近で干渉するか、
        // あるいは x=1 の列から下まで降りて、最後に左 (x=0) へ接地スライドして入る。
        board.cells[1][0] = Some(player_color(0));

        let path = AutoAi::simulate_drop_with_tuck_path(&board, 0, &TrominoKind::Corner, 3, 0, &[], &[]);
        assert!(path.is_some(), "Should find path into alcove for corner tromino");
        let (landing_y, waypoints) = path.unwrap();
        assert_eq!(landing_y, 0, "Should land at y=0 inside alcove");
        assert!(!waypoints.is_empty(), "Should generate waypoints to guide lateral slide");
    }

    #[test]
    fn test_straight_aerial_slide_under_overhang() {
        // 上側にブロックがある状態にI字（Straight）を空中横移動で埋めるテスト
        let mut board = GlobalBoard::default();
        // x=0..=2 の y=2 に屋根（オーバーハング）を配置
        // y=0, 1 は空洞
        board.cells[2][0] = Some(player_color(0));
        board.cells[2][1] = Some(player_color(0));
        board.cells[2][2] = Some(player_color(0));

        // 隣のレーン (x=3) などから進入可能
        // Straight (横向き: rot=0, (0,0), (1,0), (2,0)) を y=0, x=0 の奥に埋める
        let path = AutoAi::simulate_drop_with_tuck_path(&board, 0, &TrominoKind::Straight, 0, 0, &[], &[]);
        assert!(path.is_some(), "Should find aerial tuck-in path for Straight tromino under overhang");
        let (landing_y, waypoints) = path.unwrap();
        assert_eq!(landing_y, 0, "Should reach bottom y=0 under the roof");
        assert!(waypoints.len() >= 2, "Should have descent then lateral move waypoints");
    }

    #[test]
    fn test_corner_avoids_capping_vertical_well() {
        // レーン0 (x=0,1,2)
        // x=1 に深さ2の縦穴（左右 x=0, x=2 にブロックが積み上がっている）
        let mut board = GlobalBoard::default();
        board.cells[0][0] = Some(player_color(0));
        board.cells[1][0] = Some(player_color(0));
        board.cells[0][2] = Some(player_color(0));
        board.cells[1][2] = Some(player_color(0));
        // x=1 は y=0,1 ともに空洞（深さ2の縦穴）
        let wells = board.find_lane_vertical_wells(0);
        assert_eq!(wells.len(), 1);
        assert_eq!(wells[0], (1, 0, 2));

        let signals = crate::game::LaneSignalBoard::default();
        // L字（Corner）を落とす際、x=1 の縦穴の開口部（y=1以上）にフタをして下の空洞を埋められなくする手を避ける
        let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Corner, &[], &[], &signals)
            .expect("Move found for Corner");

        let offsets = TrominoKind::Corner.cell_offsets(best_move.rotation);
        // x=1 にフタ（y > 0 で x=1 を塞いで y=0 を空洞のまま残す）をしていないことを検証
        let caps_well = offsets.iter().any(|(dx, dy)| {
            (best_move.target_x + dx) == 1 && (best_move.landing_y + dy) > 0
        });
        assert!(!caps_well, "L-tromino must not cap the vertical well at x=1 leaving a hole below");
    }

    #[test]
    fn test_straight_cooperatively_fills_neighbor_vertical_well() {
        // レーン1 (x=4,5,6) に深さ2の縦穴（例えば x=5 が空洞、x=4,6 に高さ2のブロック）
        let mut board = GlobalBoard::default();
        board.cells[0][4] = Some(player_color(1));
        board.cells[1][4] = Some(player_color(1));
        board.cells[0][6] = Some(player_color(1));
        board.cells[1][6] = Some(player_color(1));

        let mut signals = crate::game::LaneSignalBoard::default();
        // レーン1が深さ2の縦穴 (x=5, bottom_y=0, depth=2) をシグナルで通知
        signals.signals[1].vertical_well = Some((5, 0, 2));

        // レーン0 (x=0,1,2) に I字（Straight）が出現
        // レーン0のプレイヤーは隣接するレーン1の縦穴 (x=5) に縦向き (rot=1) で駆けつけ、
        // 縦穴をピッタリ埋める協調手を選択する
        let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Straight, &[], &[], &signals)
            .expect("Move found for Straight");

        assert_eq!(best_move.rotation % 2, 1, "Should choose vertical orientation (Straight rot=1)");
        assert_eq!(best_move.target_x, 5, "Should place at neighbor lane's vertical well at x=5");
        assert_eq!(best_move.landing_y, 0, "Should sink to bottom y=0 to fill the well");
    }

    #[test]
    fn test_straight_avoids_vertical_barrier_on_border() {
        // レーン0 (x=0,1,2) と 境界列 (x=3)
        // 境界列 x=3 にすでにブロックが1つある場合、そこに縦I字を重ねて壁化することを避ける
        let mut board = GlobalBoard::default();
        board.cells[0][3] = Some(player_color(0));

        let signals = crate::game::LaneSignalBoard::default();
        let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Straight, &[], &[], &signals)
            .expect("Move found for Straight");

        // 境界列 x=3 に縦向き (rot=1) で置いて壁を伸ばす悪手を選択していないか検証
        let is_vertical_on_border = (best_move.rotation % 2 == 1) && best_move.target_x == 3;
        assert!(!is_vertical_on_border, "AI must avoid placing vertical I-tromino on border column to form a barrier");
    }

    #[test]
    fn test_ai_avoids_creating_deep_vertical_well() {
        // レーン0 (x=0,1,2) で平坦な地面 (y=0) があるとき、
        // 縦向きのI字 (1x3) を端に立てて隣に深さ3の縦穴を不必要に作り出す手を避け、
        // 平坦に横置き (rot=0) して縦穴形成を防ぐことを検証
        let board = GlobalBoard::default();
        let signals = crate::game::LaneSignalBoard::default();

        let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Straight, &[], &[], &signals)
            .expect("Move found for Straight");

        // 完全に平坦な盤面では、縦向き (rot=1) で自ら深さ3の溝を作るのではなく、
        // 横向き (rot=0) で平坦に接地させるはず
        assert_eq!(best_move.rotation % 2, 0, "AI should prefer horizontal I-tromino on flat board to prevent creating deep vertical wells");
    }

    #[test]
    fn test_prioritizes_filling_well_even_with_unreachable_alcove() {
        // レーン0 (x=0,1,2)
        // x=1 に深さ2の縦穴があり、その底の横 (x=0, y=0) に高さ1マスの空間（横穴）がある状況
        // x=0 の上 (y=1..LANE_HEIGHT) はブロックで塞がれている
        // この横空間 (x=0, y=0) は高さ1で、I字（高さ3）は入れず、L字も幅1の縦穴を通れない。
        let mut board = GlobalBoard::default();
        for y in 1..4 {
            board.cells[y][0] = Some(player_color(0)); // x=0 の上はブロック（y=0のみ空洞）
        }
        for y in 0..4 {
            board.cells[y][2] = Some(player_color(0)); // x=2 はブロック壁
        }
        // x=1 は y=0,1 が空洞（深さ4の縦穴）
        assert!(board.is_unreachable_alcove(0, 0), "x=0, y=0 should be detected as unreachable alcove");

        let signals = crate::game::LaneSignalBoard::default();
        // ここで I字（Straight）を落とす際、横のどうしようもない空洞 (0,0) を気にして縦穴埋めを躊躇するのではなく、
        // 縦向き (rot=1) で x=1 の縦穴を埋めることを優先する
        let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Straight, &[], &[], &signals)
            .expect("Move found for Straight");

        assert_eq!(best_move.rotation % 2, 1, "Should choose vertical Straight");
        assert_eq!(best_move.target_x, 1, "Should fill the vertical well at x=1");
    }

    #[test]
    fn test_mid_air_replanning_on_unexpected_obstacle() {
        // 落下途中で目標地点が塞がれた・状況が変化した場合の空中再計算テスト
        let mut board = GlobalBoard::default();
        // 当初の目標地点 (x=0, y=0) が急に他者によって埋まった状況
        board.cells[0][0] = Some(player_color(1));

        let kind = TrominoKind::Straight;
        let signals = crate::game::LaneSignalBoard::default();

        // 空中 (current_x=0, current_y=10, current_rotation=1) にいるトミノが再計算
        let re_eval = AutoAi::find_best_move_from_position(
            &board,
            0,
            &kind,
            0,
            10,
            1,
            &[],
            &[],
            &signals,
        ).expect("Replanned move found");

        // 埋まってしまった (x=0, y=0) への衝突を避け、安全な別の着地点（例えば x=1 や x=2 など）へ目標変更すること
        let collides_with_obstacle = re_eval.target_x == 0 && re_eval.landing_y == 0;
        assert!(!collides_with_obstacle, "AI must replan to a safe target avoiding the unexpected obstacle");
    }
}
