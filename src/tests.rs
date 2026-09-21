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

        let best_move = AutoAi::find_best_move(&board, 0, &kind, &predicted_others, &[])
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
        let best_move = AutoAi::find_best_move(&board, 0, &kind, &[], &[]).expect("Move found");

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
}
