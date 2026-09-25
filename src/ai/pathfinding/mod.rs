pub mod aerial;
pub mod common;
pub mod spawn_drop;

use crate::board::GlobalBoard;
use crate::tromino::TrominoKind;

#[allow(unused_imports)]
pub use aerial::simulate_drop_from_path;
#[allow(unused_imports)]
pub use common::can_place_check;
#[allow(unused_imports)]
pub use spawn_drop::{simulate_direct_drop, simulate_drop_with_tuck_path};

pub struct PathFinder;

impl PathFinder {
    #[allow(dead_code)]
    #[inline]
    pub fn can_place_check(
        board: &GlobalBoard,
        kind: &TrominoKind,
        rot: usize,
        base_x: i32,
        base_y: i32,
        obstacles: &[(i32, i32)],
    ) -> bool {
        common::can_place_check(board, kind, rot, base_x, base_y, obstacles)
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn simulate_drop_from_path(
        board: &GlobalBoard,
        kind: &TrominoKind,
        from_rot: usize,
        to_rot: usize,
        start_x: i32,
        start_y: i32,
        target_x: i32,
        reserved_landing_cells: &[(i32, i32)],
    ) -> Option<(i32, Vec<(i32, i32)>)> {
        aerial::simulate_drop_from_path(
            board,
            kind,
            from_rot,
            to_rot,
            start_x,
            start_y,
            target_x,
            reserved_landing_cells,
        )
    }

    #[allow(dead_code)]
    #[inline]
    pub fn simulate_drop_with_tuck(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        rot: usize,
        target_x: i32,
        reserved_landing_cells: &[(i32, i32)],
        air_obstacles: &[(i32, i32)],
    ) -> Option<i32> {
        spawn_drop::simulate_drop_with_tuck_path(
            board,
            lane_id,
            kind,
            rot,
            target_x,
            reserved_landing_cells,
            air_obstacles,
        )
        .map(|(y, _)| y)
    }

    #[inline]
    pub fn simulate_drop_with_tuck_path(
        board: &GlobalBoard,
        lane_id: usize,
        kind: &TrominoKind,
        rot: usize,
        target_x: i32,
        reserved_landing_cells: &[(i32, i32)],
        air_obstacles: &[(i32, i32)],
    ) -> Option<(i32, Vec<(i32, i32)>)> {
        spawn_drop::simulate_drop_with_tuck_path(
            board,
            lane_id,
            kind,
            rot,
            target_x,
            reserved_landing_cells,
            air_obstacles,
        )
    }
}
