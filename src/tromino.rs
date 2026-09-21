use bevy_prng::WyRand;
use rand_core::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrominoKind {
    Straight,
    Corner,
}

impl TrominoKind {
    pub fn random_from_rng(rng: &mut WyRand) -> Self {
        if rng.next_u32() % 2 == 0 {
            TrominoKind::Straight
        } else {
            TrominoKind::Corner
        }
    }

    /// 各種別の可能な回転数
    pub fn rotation_count(&self) -> usize {
        match self {
            TrominoKind::Straight => 2, // 0: 横 (3x1), 1: 縦 (1x3)
            TrominoKind::Corner => 4,   // 4方向
        }
    }

    /// (dx, dy) の相対セルオフセット (原点 (0,0) を基準とする)
    /// x: 0..width, y: 0..height
    pub fn cell_offsets(&self, rotation: usize) -> Vec<(i32, i32)> {
        match self {
            TrominoKind::Straight => match rotation % 2 {
                // 横向き: [■ ■ ■]
                0 => vec![(0, 0), (1, 0), (2, 0)],
                // 縦向き:
                // [■]
                // [■]
                // [■]
                1 => vec![(0, 0), (0, 1), (0, 2)],
                _ => unreachable!(),
            },
            TrominoKind::Corner => match rotation % 4 {
                // 0: ■ .
                //    ■ ■
                0 => vec![(0, 1), (0, 0), (1, 0)],
                // 1: ■ ■
                //    ■ .
                1 => vec![(0, 1), (1, 1), (0, 0)],
                // 2: ■ ■
                //    . ■
                2 => vec![(0, 1), (1, 1), (1, 0)],
                // 3: . ■
                //    ■ ■
                3 => vec![(1, 1), (0, 0), (1, 0)],
                _ => unreachable!(),
            },
        }
    }
}
