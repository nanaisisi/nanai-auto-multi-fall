use crate::ai::{AutoAi, PredictedPlacement};
use crate::board::{GlobalBoard, is_border_column};
use crate::config::{TOTAL_GRID_WIDTH, player_color};
use crate::tromino::TrominoKind;

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
    println!("DEBUG: running find_best_move");
    let best_move = AutoAi::find_best_move(&board, 0, &kind, &predicted_others, &[], &signals)
        .expect("Move found");
    println!("DEBUG: best_move = {:?}", best_move);

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
    let best_move =
        AutoAi::find_best_move(&board, 0, &kind, &[], &[], &signals).expect("Move found");

    let offsets = kind.cell_offsets(best_move.rotation);
    let uses_border = offsets
        .iter()
        .any(|(dx, _)| is_border_column((best_move.target_x + dx) as usize));

    assert!(
        uses_border,
        "AI should utilize border columns when appropriate"
    );
}

#[test]
fn test_ai_handles_high_placement_without_panic() {
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
fn test_corner_avoids_capping_vertical_well() {
    let mut board = GlobalBoard::default();
    board.cells[0][0] = Some(player_color(0));
    board.cells[1][0] = Some(player_color(0));
    board.cells[0][2] = Some(player_color(0));
    board.cells[1][2] = Some(player_color(0));

    let wells = board.find_lane_vertical_wells(0);
    assert_eq!(wells.len(), 1);
    assert_eq!(wells[0], (1, 0, 2));

    let signals = crate::game::LaneSignalBoard::default();
    let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Corner, &[], &[], &signals)
        .expect("Move found for Corner");

    let offsets = TrominoKind::Corner.cell_offsets(best_move.rotation);
    let caps_well = offsets
        .iter()
        .any(|(dx, dy)| (best_move.target_x + dx) == 1 && (best_move.landing_y + dy) > 0);
    assert!(
        !caps_well,
        "L-tromino must not cap the vertical well at x=1 leaving a hole below"
    );
}

#[test]
fn test_straight_cooperatively_fills_neighbor_vertical_well() {
    let mut board = GlobalBoard::default();
    board.cells[0][4] = Some(player_color(1));
    board.cells[1][4] = Some(player_color(1));
    board.cells[0][6] = Some(player_color(1));
    board.cells[1][6] = Some(player_color(1));

    let mut signals = crate::game::LaneSignalBoard::default();
    signals.signals[1].vertical_well = Some((5, 0, 2));

    let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Straight, &[], &[], &signals)
        .expect("Move found for Straight");

    assert_eq!(
        best_move.rotation % 2,
        1,
        "Should choose vertical orientation (Straight rot=1)"
    );
    assert_eq!(
        best_move.target_x, 5,
        "Should place at neighbor lane's vertical well at x=5"
    );
    assert_eq!(
        best_move.landing_y, 0,
        "Should sink to bottom y=0 to fill the well"
    );
}

#[test]
fn test_straight_avoids_vertical_barrier_on_border() {
    let mut board = GlobalBoard::default();
    board.cells[0][3] = Some(player_color(0));

    let signals = crate::game::LaneSignalBoard::default();
    let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Straight, &[], &[], &signals)
        .expect("Move found for Straight");

    let is_vertical_on_border = (best_move.rotation % 2 == 1) && best_move.target_x == 3;
    assert!(
        !is_vertical_on_border,
        "AI must avoid placing vertical I-tromino on border column to form a barrier"
    );
}

#[test]
fn test_ai_avoids_creating_deep_vertical_well() {
    let board = GlobalBoard::default();
    let signals = crate::game::LaneSignalBoard::default();

    let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Straight, &[], &[], &signals)
        .expect("Move found for Straight");

    assert_eq!(
        best_move.rotation % 2,
        0,
        "AI should prefer horizontal I-tromino on flat board to prevent creating deep vertical wells"
    );
}

#[test]
fn test_prioritizes_filling_well_even_with_unreachable_alcove() {
    let mut board = GlobalBoard::default();
    for y in 1..4 {
        board.cells[y][0] = Some(player_color(0));
    }
    for y in 0..4 {
        board.cells[y][2] = Some(player_color(0));
    }
    assert!(
        board.is_unreachable_alcove(0, 0),
        "x=0, y=0 should be detected as unreachable alcove"
    );

    let signals = crate::game::LaneSignalBoard::default();
    let best_move = AutoAi::find_best_move(&board, 0, &TrominoKind::Straight, &[], &[], &signals)
        .expect("Move found for Straight");

    assert_eq!(best_move.rotation % 2, 1, "Should choose vertical Straight");
    assert_eq!(
        best_move.target_x, 1,
        "Should fill the vertical well at x=1"
    );
}

#[test]
fn test_mid_air_replanning_on_unexpected_obstacle() {
    let mut board = GlobalBoard::default();
    board.cells[0][0] = Some(player_color(1));

    let kind = TrominoKind::Straight;
    let signals = crate::game::LaneSignalBoard::default();

    let re_eval = AutoAi::find_best_move_from_position(
        &board,
        0,
        &kind,
        0,
        10,
        1,
        Some((0, 1)),
        &[],
        &[],
        &signals,
    )
    .expect("Replanned move found");

    let collides_with_obstacle = re_eval.target_x == 0 && re_eval.landing_y == 0;
    assert!(
        !collides_with_obstacle,
        "AI must replan to a safe target avoiding the unexpected obstacle"
    );
}
