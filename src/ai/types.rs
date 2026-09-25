use crate::game::LanePace;
use crate::tromino::TrominoKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct ScoreBreakdown {
    pub total: f32,
    pub lines: f32,
    pub coop_lines: f32,
    pub holes_penalty: f32,
    pub height_penalty: f32,
    pub bumpiness_penalty: f32,
    pub anti_roof: f32,
    pub hole_fill: f32,
    pub border: f32,
    pub signal_coop: f32,
    pub well_coop: f32,
}

impl std::fmt::Display for ScoreBreakdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "total:{:+.1} (line:{:+.0}, coop:{:+.0}, holes:{:+.1}, h:{:+.1}, bump:{:+.1}, roof:{:+.0}, fill:{:+.0}, border:{:+.0}, well:{:+.0}, sig:{:+.0})",
            self.total,
            self.lines,
            self.coop_lines,
            self.holes_penalty,
            self.height_penalty,
            self.bumpiness_penalty,
            self.anti_roof,
            self.hole_fill,
            self.border,
            self.well_coop,
            self.signal_coop,
        )
    }
}

#[derive(Debug, Clone)]
pub struct MoveEvaluation {
    pub rotation: usize,
    pub target_x: i32,
    pub landing_y: i32,
    pub score: f32,
    pub breakdown: ScoreBreakdown,
    pub pace: LanePace,
    pub waypoints: Vec<(i32, i32)>,
}

/// 他プレイヤーの着地予測情報
#[derive(Debug, Clone, Copy)]
pub struct PredictedPlacement {
    pub player_id: usize,
    pub kind: TrominoKind,
    pub rotation: usize,
    pub target_x: i32,
    pub landing_y: i32,
}
