use crate::board::GlobalBoard;
use crate::tromino::TrominoKind;
use bevy::prelude::*;

/// 指定した (x, y, rotation) においてトミノが盤面（壁・固定ブロック）および他トミノと衝突するか判定
pub fn check_position_collision(
    board: &GlobalBoard,
    kind: &TrominoKind,
    rotation: usize,
    x: i32,
    y: i32,
    this_entity: Entity,
    current_falling: &[(Entity, Vec<(i32, i32)>)],
) -> bool {
    let offsets = kind.cell_offsets(rotation);

    for (dx, dy) in &offsets {
        let cx = x + dx;
        let cy = y + dy;

        // 1. 盤面の壁・固定ブロックとの衝突
        if board.is_occupied(cx, cy) {
            return true;
        }

        // 2. 落下中の他のトミノとの衝突
        for (other_entity, other_cells) in current_falling {
            if *other_entity == this_entity {
                continue;
            }
            if other_cells.contains(&(cx, cy)) {
                return true;
            }
        }
    }

    false
}

/// 回転衝突判定＆壁キック（Wall Kick）処理
/// 回転先が塞がれている場合、キック候補 (dx, dy) を試して安全な位置へシフト。
/// どこにもキックできない場合は None（回転失敗）を返す
#[allow(clippy::too_many_arguments)]
pub fn try_rotate_with_kick(
    board: &GlobalBoard,
    kind: &TrominoKind,
    from_rot: usize,
    to_rot: usize,
    cur_x: i32,
    cur_y: i32,
    this_entity: Entity,
    current_falling: &[(Entity, Vec<(i32, i32)>)],
) -> Option<(i32, i32)> {
    if from_rot == to_rot {
        return Some((cur_x, cur_y));
    }

    // キック候補オフセット: (0, 0) その場 -> (-1, 0) 左 -> (+1, 0) 右 -> (0, +1) 上 -> (-1, +1) -> (+1, +1)
    let kick_offsets = [(0, 0), (-1, 0), (1, 0), (0, 1), (-1, 1), (1, 1), (0, -1)];

    for (kdx, kdy) in kick_offsets {
        let test_x = cur_x + kdx;
        let test_y = cur_y + kdy;

        let collides = check_position_collision(
            board,
            kind,
            to_rot,
            test_x,
            test_y,
            this_entity,
            current_falling,
        );

        if !collides {
            return Some((test_x, test_y));
        }
    }

    None
}
