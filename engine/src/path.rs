//! Reachability to goal — **BFS** on the 9×9 pawn grid (uniform edge cost).
//!
//! This is the standard approach (same family as scraped JS `isWallBlocking` and
//! pavlosdais/Quoridor path checks). Dijkstra is unnecessary; DFS works but BFS
//! also yields shortest-path distance for eval.

use crate::board::{Board, Player};
use crate::grid::{can_step, is_goal, square_index, unpack_square};

const NEIGHBORS: [(i8, i8); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// Stack BFS with `u128` visited bitset — fastest generic reachability on 81 cells.
#[inline]
pub fn can_reach_goal(board: &Board, player: Player) -> bool {
    shortest_distance(board, player).is_some()
}

/// `None` if unreachable; otherwise distance in pawn steps to any goal square on that row.
pub fn shortest_distance(board: &Board, player: Player) -> Option<u8> {
    let (sr, sc) = board.pawn(player);
    let _goal = crate::grid::goal_row(player);
    let start = square_index(sr, sc);
    let mut visited: u128 = 1u128 << start;
    let mut queue: [u8; 81] = [0; 81];
    let mut depth: [u8; 81] = [0; 81];
    let mut head = 0usize;
    let mut tail = 0usize;
    queue[tail] = start;
    depth[tail] = 0;
    tail += 1;

    while head < tail {
        let sq = queue[head];
        let d = depth[head];
        head += 1;
        let (r, c) = unpack_square(sq);
        if is_goal(player, r) {
            return Some(d);
        }
        for (dr, dc) in NEIGHBORS {
            if !can_step(board, r, c, dr, dc) {
                continue;
            }
            let nr = (r as i8 + dr) as u8;
            let nc = (c as i8 + dc) as u8;
            let nsq = square_index(nr, nc);
            let mask = 1u128 << nsq;
            if visited & mask != 0 {
                continue;
            }
            visited |= mask;
            queue[tail] = nsq;
            depth[tail] = d + 1;
            tail += 1;
        }
    }
    None
}

/// BFS for both players — used when testing wall placement (hot loop).
#[inline]
pub fn both_players_reach_goals(board: &Board) -> bool {
    can_reach_goal(board, Player::One) && can_reach_goal(board, Player::Two)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::WallOrientation;
    use crate::grid::set_wall;

    #[test]
    fn start_position_reachable() {
        let board = Board::new();
        assert!(can_reach_goal(&board, Player::One));
        assert!(can_reach_goal(&board, Player::Two));
        assert_eq!(shortest_distance(&board, Player::One), Some(8));
        assert_eq!(shortest_distance(&board, Player::Two), Some(8));
    }

    #[test]
    fn full_barrier_blocks_p1() {
        let mut board = Board::new();
        for c in 0..8u8 {
            set_wall(
                &mut board,
                6,
                c,
                WallOrientation::Horizontal,
                true,
            );
        }
        assert!(!can_reach_goal(&board, Player::One));
    }
}
