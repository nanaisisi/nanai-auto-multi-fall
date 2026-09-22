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
        let speed_after_5_lines = settings.current_base_fall_interval(5);
        let speed_after_20_lines = settings.current_base_fall_interval(20);

        assert!(speed_after_5_lines < initial_speed, "Speed should accelerate as lines are cleared");
        assert!(speed_after_20_lines <= speed_after_5_lines);
        assert!(speed_after_20_lines >= settings.min_fall_interval);

        // SoftDrop 時はさらに倍率がかかり高速
        let soft_drop_interval = speed_after_5_lines * settings.soft_drop_multiplier;
        assert!(soft_drop_interval < speed_after_5_lines);
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
}
