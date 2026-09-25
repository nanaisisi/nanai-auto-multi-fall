use crate::board::GlobalBoard;
use crate::tromino::TrominoKind;

pub fn can_place_check(
    board: &GlobalBoard,
    kind: &TrominoKind,
    rot: usize,
    base_x: i32,
    base_y: i32,
    obstacles: &[(i32, i32)],
) -> bool {
    if !board.can_place(kind, rot, base_x, base_y) {
        return false;
    }
    for (dx, dy) in kind.cell_offsets(rot) {
        let cx = base_x + dx;
        let cy = base_y + dy;
        if obstacles.contains(&(cx, cy)) {
            return false;
        }
    }
    true
}

/// 探索キュー復元用の親ノードマップから折れ曲がり地点（ウェイポイント）を抽出
pub fn reconstruct_waypoints(
    parent_map: &std::collections::HashMap<(i32, i32), (i32, i32)>,
    target_x: i32,
    best_y: i32,
) -> Vec<(i32, i32)> {
    let mut raw_path = Vec::new();
    let mut curr = (target_x, best_y);
    raw_path.push(curr);
    while let Some(&prev) = parent_map.get(&curr) {
        raw_path.push(prev);
        curr = prev;
    }
    raw_path.reverse();

    let mut waypoints = Vec::new();
    if raw_path.len() <= 2 {
        for pt in raw_path {
            if !waypoints.contains(&pt) {
                waypoints.push(pt);
            }
        }
    } else {
        let mut prev_dir = (raw_path[1].0 - raw_path[0].0, raw_path[1].1 - raw_path[0].1);
        for i in 2..raw_path.len() {
            let cur_dir = (
                raw_path[i].0 - raw_path[i - 1].0,
                raw_path[i].1 - raw_path[i - 1].1,
            );
            if cur_dir != prev_dir {
                waypoints.push(raw_path[i - 1]);
                prev_dir = cur_dir;
            }
        }
        waypoints.push((target_x, best_y));
    }

    if waypoints.is_empty() {
        waypoints.push((target_x, best_y));
    }

    waypoints
}
