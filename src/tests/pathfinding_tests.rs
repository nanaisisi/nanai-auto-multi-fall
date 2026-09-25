use crate::ai::AutoAi;
use crate::board::GlobalBoard;
use crate::config::player_color;
use crate::tromino::TrominoKind;

#[test]
fn test_tuck_in_pathfinding_under_overhang() {
    let mut board = GlobalBoard::default();
    board.cells[2][1] = Some(player_color(0));

    let kind = TrominoKind::Straight;
    let direct_y = AutoAi::simulate_drop_with_tuck(&board, 0, &kind, 1, 1, &[], &[]);
    assert!(direct_y.is_some());
}

#[test]
fn test_corner_ground_slide_into_alcove() {
    let mut board = GlobalBoard::default();
    board.cells[1][0] = Some(player_color(0));

    let path =
        AutoAi::simulate_drop_with_tuck_path(&board, 0, &TrominoKind::Corner, 3, 0, &[], &[]);
    assert!(
        path.is_some(),
        "Should find path into alcove for corner tromino"
    );
    let (landing_y, waypoints) = path.unwrap();
    assert_eq!(landing_y, 0, "Should land at y=0 inside alcove");
    assert!(
        !waypoints.is_empty(),
        "Should generate waypoints to guide lateral slide"
    );
}

#[test]
fn test_straight_aerial_slide_under_overhang() {
    let mut board = GlobalBoard::default();
    board.cells[2][0] = Some(player_color(0));
    board.cells[2][1] = Some(player_color(0));
    board.cells[2][2] = Some(player_color(0));

    let path =
        AutoAi::simulate_drop_with_tuck_path(&board, 0, &TrominoKind::Straight, 0, 0, &[], &[]);
    assert!(
        path.is_some(),
        "Should find aerial tuck-in path for Straight tromino under overhang"
    );
    let (landing_y, waypoints) = path.unwrap();
    assert_eq!(landing_y, 0, "Should reach bottom y=0 under the roof");
    assert!(
        waypoints.len() >= 2,
        "Should have descent then lateral move waypoints"
    );
}
