//! Root wall cap — keep all pawns, all CAT-hot walls, then hottest cold walls up to cap.

use crate::cat::constants::CAT_HOT_CM;
use crate::cat::CorridorAttention;
use crate::core::board::Move;
use crate::movegen::MAX_LEGAL_MOVES;

/// Indices of walls kept when capping to `max_walls` hottest by CAT (pawns always kept).
/// Every wall with edge heat ≥ `CAT_HOT_CM` is always kept — pierce must not drop corridor walls.
pub fn root_wall_keep_mask(
    buf: &[Move],
    n: usize,
    cat: &CorridorAttention,
    max_walls: usize,
) -> [bool; MAX_LEGAL_MOVES] {
    let mut keep = [true; MAX_LEGAL_MOVES];
    let mut ranked = [(0usize, 0u16); MAX_LEGAL_MOVES];
    let mut wall_count = 0usize;
    for i in 0..n {
        if let Move::Wall {
            row,
            col,
            orientation,
        } = buf[i]
        {
            let heat = cat.wall_edge_heat(row, col, orientation);
            ranked[wall_count] = (i, heat);
            wall_count += 1;
            keep[i] = heat >= CAT_HOT_CM;
        }
    }
    if wall_count <= max_walls {
        return keep;
    }
    let mandatory = (0..n)
        .filter(|&i| matches!(buf[i], Move::Wall { .. }) && keep[i])
        .count();
    if mandatory >= max_walls {
        return keep;
    }
    ranked[..wall_count].sort_by(|a, b| b.1.cmp(&a.1));
    let mut kept = mandatory;
    for &(i, _) in &ranked[..wall_count] {
        if keep[i] {
            continue;
        }
        if kept >= max_walls {
            break;
        }
        keep[i] = true;
        kept += 1;
    }
    keep
}

/// Keep every pawn; retain CAT-hot walls plus hottest cold walls up to `max_walls`.
pub fn cap_root_wall_moves(
    buf: &mut [Move],
    n: &mut usize,
    cat: &CorridorAttention,
    max_walls: usize,
) {
    if *n == 0 {
        return;
    }
    let keep = root_wall_keep_mask(buf, *n, cat, max_walls);
    let mut out = 0usize;
    for i in 0..*n {
        if keep[i] {
            buf[out] = buf[i];
            out += 1;
        }
    }
    *n = out;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::board::{Board, WallOrientation};
    use crate::path::BfsScratch;

    #[test]
    fn pierce_cap_never_drops_cat_hot_walls() {
        let mut board = Board::new();
        for m in ["e2", "e8", "e3", "e7", "e4", "e6"] {
            board.apply_algebraic(m);
        }
        let mut bfs = BfsScratch::new();
        let cat = bfs.build_corridor_attention(&board);
        let walls = [
            Move::Wall {
                row: 3,
                col: 3,
                orientation: WallOrientation::Horizontal,
            },
            Move::Wall {
                row: 4,
                col: 3,
                orientation: WallOrientation::Horizontal,
            },
        ];
        let keep = root_wall_keep_mask(&walls, walls.len(), &cat, 2);
        assert!(keep[0], "d4h must stay when CAT-hot");
        assert!(keep[1], "d5h must stay when CAT-hot");
    }
}
