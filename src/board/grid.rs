use crate::config::{LANE_HEIGHT, TOTAL_GRID_WIDTH};
use bevy::prelude::*;

#[derive(Resource, Debug, Clone)]
pub struct GlobalBoard {
    // [y][x] : y=0 が最下段、y=LANE_HEIGHT-1 が最上段
    pub cells: [[Option<Color>; TOTAL_GRID_WIDTH]; LANE_HEIGHT],
    pub score: u32,
    pub lines_cleared: u32,
    pub game_over: bool,
    pub lane_stuck: [bool; crate::config::LANE_COUNT],
    /// 盤面の固定・消去が発生するたびにインクリメントされるバージョン番号
    pub board_version: u64,
}

impl Default for GlobalBoard {
    fn default() -> Self {
        Self {
            cells: [[None; TOTAL_GRID_WIDTH]; LANE_HEIGHT],
            score: 0,
            lines_cleared: 0,
            game_over: false,
            lane_stuck: [false; crate::config::LANE_COUNT],
            board_version: 0,
        }
    }
}

impl GlobalBoard {
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.cells = [[None; TOTAL_GRID_WIDTH]; LANE_HEIGHT];
        self.game_over = false;
        self.lane_stuck = [false; crate::config::LANE_COUNT];
    }

    /// 指定レーンをスタック状態にし、全レーンがスタックした場合はゲームオーバーフラグを立てる。
    /// 全レーンがスタックしたかどうか（ゲームオーバーになったか）を返す。
    pub fn mark_lane_stuck(&mut self, lane_id: usize) -> bool {
        self.lane_stuck[lane_id] = true;
        let all_stuck = self.lane_stuck.iter().all(|&stuck| stuck);
        if all_stuck {
            self.game_over = true;
        }
        all_stuck
    }
}
