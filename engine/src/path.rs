//! Reachability to goal — **BFS** on the 9×9 pawn grid (uniform edge cost).
//!
//! This is the standard approach (same family as scraped JS `isWallBlocking` and
//! pavlosdais/Quoridor path checks). Dijkstra is unnecessary; DFS works but BFS
//! also yields shortest-path distance for eval.

use crate::board::{Board, Player};
use crate::grid::{can_step, is_goal, square_index, unpack_square};

const NEIGHBORS: [(i8, i8); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// Reused BFS queue + visited bitset — pass through perft/move-gen hot loops.
#[derive(Clone)]
pub struct BfsScratch {
    visited: u128,
    queue: [u8; 81],
    depth: [u8; 81],
}

impl Default for BfsScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl BfsScratch {
    pub fn new() -> Self {
        Self {
            visited: 0,
            queue: [0; 81],
            depth: [0; 81],
        }
    }

    /// Reachability only — no per-node depth array (hot wall-legality path).
    #[inline]
    pub fn can_reach_goal(&mut self, board: &Board, player: Player) -> bool {
        let (sr, sc) = board.pawn(player);
        let start = square_index(sr, sc);
        self.visited = 1u128 << start;
        let mut head = 0usize;
        let mut tail = 1usize;
        self.queue[0] = start;

        while head < tail {
            let sq = self.queue[head];
            head += 1;
            let (r, c) = unpack_square(sq);
            if is_goal(player, r) {
                return true;
            }
            for (dr, dc) in NEIGHBORS {
                if !can_step(board, r, c, dr, dc) {
                    continue;
                }
                let nr = (r as i8 + dr) as u8;
                let nc = (c as i8 + dc) as u8;
                let nsq = square_index(nr, nc);
                let mask = 1u128 << nsq;
                if self.visited & mask != 0 {
                    continue;
                }
                self.visited |= mask;
                self.queue[tail] = nsq;
                tail += 1;
            }
        }
        false
    }

    /// Short-circuit: stop after first player fails.
    #[inline]
    pub fn both_players_reach_goals(&mut self, board: &Board) -> bool {
        self.can_reach_goal(board, Player::One) && self.can_reach_goal(board, Player::Two)
    }

    /// Distance BFS — uses `depth` scratch for eval.
    pub fn shortest_distance(&mut self, board: &Board, player: Player) -> Option<u8> {
        let (sr, sc) = board.pawn(player);
        let start = square_index(sr, sc);
        self.visited = 1u128 << start;
        let mut head = 0usize;
        let mut tail = 1usize;
        self.queue[0] = start;
        self.depth[0] = 0;

        while head < tail {
            let sq = self.queue[head];
            let d = self.depth[head];
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
                if self.visited & mask != 0 {
                    continue;
                }
                self.visited |= mask;
                self.queue[tail] = nsq;
                self.depth[tail] = d + 1;
                tail += 1;
            }
        }
        None
    }
}

/// Stack BFS with `u128` visited bitset — convenience for tests and one-off calls.
#[inline]
pub fn can_reach_goal(board: &Board, player: Player) -> bool {
    BfsScratch::new().can_reach_goal(board, player)
}

/// `None` if unreachable; otherwise distance in pawn steps to any goal square on that row.
pub fn shortest_distance(board: &Board, player: Player) -> Option<u8> {
    BfsScratch::new().shortest_distance(board, player)
}

/// BFS for both players — used when testing wall placement (hot loop).
#[inline]
pub fn both_players_reach_goals(board: &Board) -> bool {
    BfsScratch::new().both_players_reach_goals(board)
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

    #[test]
    fn scratch_matches_stack_bfs() {
        let board = Board::new();
        let mut scratch = BfsScratch::new();
        assert_eq!(
            scratch.shortest_distance(&board, Player::One),
            Some(8)
        );
        assert!(scratch.both_players_reach_goals(&board));
    }
}
