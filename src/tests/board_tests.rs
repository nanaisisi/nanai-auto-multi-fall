use crate::board::{GlobalBoard, is_border_column};
use crate::config::{SPAWN_WALL_MIN_Y, TOTAL_GRID_WIDTH, player_color};
use crate::tromino::TrominoKind;

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

    assert!(board.can_place(&kind, 1, 30, 0));
    assert!(!board.can_place(&kind, 0, 30, 0));
    assert!(board.can_place(&kind, 0, 28, 0));
}

#[test]
fn test_hole_detection_and_progress_pacing() {
    let mut board = GlobalBoard::default();
    board.cells[2][0] = Some(player_color(0));

    let hole = board.find_lane_deepest_hole(0);
    assert!(hole.is_some());
    let (hx, hy, has_roof) = hole.unwrap();
    assert_eq!(hx, 0);
    assert!(has_roof, "Should detect roof over hole");
    assert!(hy < 2);

    let fresh_board = GlobalBoard::default();
    assert_eq!(fresh_board.lane_fill_ratio_at_target_line(0), 0.0);

    let ratio = board.lane_fill_ratio_at_target_line(0);
    assert!((ratio - 1.0 / 3.0).abs() < 0.01);

    let kind = TrominoKind::Straight;
    let signals = crate::game::LaneSignalBoard::default();
    let best_move = crate::ai::AutoAi::find_best_move(&board, 0, &kind, &[], &[], &signals)
        .expect("Move found");
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
    assert!(speed_after_20_lines < initial_speed);
    assert!(speed_after_50_lines < speed_after_20_lines);
    assert!(speed_after_100_lines <= speed_after_50_lines);
    assert_eq!(speed_after_100_lines, settings.min_fall_interval);

    let soft_drop_interval = speed_after_20_lines * settings.soft_drop_multiplier;
    assert!(soft_drop_interval < speed_after_20_lines);
}
