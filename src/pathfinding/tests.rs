#[cfg(test)]
mod naive_reference {
    use crate::core::board::Board;
    use crate::util::grid::{can_step, is_goal, square_index, unpack_square};

    const NEIGHBORS: [(i8, i8); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

    pub fn flood_fill_naive(board: &Board, start: u8) -> u128 {
        let mut visited = 1u128 << start;
        let mut queue = [0u8; 81];
        let mut head = 0usize;
        let mut tail = 1usize;
        queue[0] = start;

        while head < tail {
            let sq = queue[head];
            head += 1;
            let (r, c) = unpack_square(sq);
            for (dr, dc) in NEIGHBORS {
                if !can_step(board, r, c, dr, dc) {
                    continue;
                }
                let nr = (r as i8 + dr) as u8;
                let nc = (c as i8 + dc) as u8;
                let nsq = square_index(nr, nc);
                let bit = 1u128 << nsq;
                if visited & bit != 0 {
                    continue;
                }
                visited |= bit;
                queue[tail] = nsq;
                tail += 1;
            }
        }
        visited
    }

    pub fn can_reach_goal_naive(board: &Board, player: crate::core::board::Player) -> bool {
        let (sr, sc) = board.pawn(player);
        let start = square_index(sr, sc);
        let mut visited = 1u128 << start;
        let mut queue = [0u8; 81];
        let mut head = 0usize;
        let mut tail = 1usize;
        queue[0] = start;

        while head < tail {
            let sq = queue[head];
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
                let bit = 1u128 << nsq;
                if visited & bit != 0 {
                    continue;
                }
                visited |= bit;
                queue[tail] = nsq;
                tail += 1;
            }
        }
        false
    }

    pub fn shortest_distance_naive(
        board: &Board,
        player: crate::core::board::Player,
    ) -> Option<u8> {
        let (sr, sc) = board.pawn(player);
        let start = square_index(sr, sc);
        let mut visited = 1u128 << start;
        let mut queue = [0u8; 81];
        let mut depth = [0u8; 81];
        let mut head = 0usize;
        let mut tail = 1usize;
        queue[0] = start;
        depth[0] = 0;

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
                let bit = 1u128 << nsq;
                if visited & bit != 0 {
                    continue;
                }
                visited |= bit;
                queue[tail] = nsq;
                depth[tail] = d + 1;
                tail += 1;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::naive_reference::{can_reach_goal_naive, flood_fill_naive, shortest_distance_naive};
    use crate::core::board::{Board, Player, WallOrientation};
    use crate::movegen::generate_legal_moves;
    use crate::pathfinding::bff::flood_fill;
    use crate::pathfinding::bfs::layers::fill_dist_to_goal_row;
    use crate::pathfinding::bfs::{can_reach_goal, shortest_distance, BfsScratch};
    use crate::pathfinding::masks::DirMasks;
    use crate::util::grid::{can_step, goal_row, set_wall, square_index, unpack_square};

    fn assert_bitwise_matches_naive(board: &Board) {
        let masks = DirMasks::from_board(board);
        let mut scratch = BfsScratch::new();

        for sq in 0u8..81 {
            let bitwise = flood_fill(sq, masks);
            let naive = flood_fill_naive(board, sq);
            assert_eq!(bitwise, naive, "reachable mismatch from sq {sq}");
        }

        for player in [Player::One, Player::Two] {
            assert_eq!(
                scratch.can_reach_goal(board, player),
                can_reach_goal_naive(board, player),
            );
            assert_eq!(
                scratch.shortest_distance(board, player),
                shortest_distance_naive(board, player),
            );
        }
    }

    fn assert_bfs_path_matches_naive(board: &Board) {
        let masks = DirMasks::from_board(board);
        let mut scratch = BfsScratch::new();
        let mut path = [u8::MAX; 81];
        let mut next = [u8::MAX; 81];
        let mut distance_to_goal = [u8::MAX; 81];

        for player in [Player::One, Player::Two] {
            let expected_distance = shortest_distance_naive(board, player);
            let path_len = scratch.shortest_path(board, player, &mut path);
            assert_eq!(
                path_len.map(|len| (len - 1) as u8),
                expected_distance,
                "shortest-path length mismatch for {player:?}"
            );

            if let Some(len) = path_len {
                let (pawn_row, pawn_col) = board.pawn(player);
                assert_eq!(path[0], square_index(pawn_row, pawn_col));
                assert_eq!(unpack_square(path[len - 1]).0, goal_row(player));

                for step in path[..len].windows(2) {
                    let (row, col) = unpack_square(step[0]);
                    let (next_row, next_col) = unpack_square(step[1]);
                    let dr = next_row as i8 - row as i8;
                    let dc = next_col as i8 - col as i8;
                    assert!(
                        can_step(board, row, col, dr, dc),
                        "illegal reconstructed step {} -> {} for {player:?}",
                        step[0],
                        step[1]
                    );
                }
            }

            scratch.fill_next_toward_goal(board, player, &mut next);
            fill_dist_to_goal_row(player, masks, &mut distance_to_goal);
            for sq in 0u8..81 {
                let distance = distance_to_goal[sq as usize];
                if distance == 0 || distance == u8::MAX {
                    assert_eq!(next[sq as usize], u8::MAX);
                    continue;
                }

                let next_sq = next[sq as usize];
                assert_ne!(next_sq, u8::MAX, "missing BFS next step from {sq}");
                assert_eq!(
                    distance_to_goal[next_sq as usize] + 1,
                    distance,
                    "next step does not descend one BFS layer from {sq}"
                );
                let (row, col) = unpack_square(sq);
                let (next_row, next_col) = unpack_square(next_sq);
                assert!(can_step(
                    board,
                    row,
                    col,
                    next_row as i8 - row as i8,
                    next_col as i8 - col as i8,
                ));
            }
        }
    }

    #[test]
    fn start_position_reachable() {
        let board = Board::new();
        assert!(can_reach_goal(&board, Player::One));
        assert_eq!(shortest_distance(&board, Player::One), Some(8));
    }

    #[test]
    fn bitwise_flood_matches_naive_on_startpos() {
        assert_bitwise_matches_naive(&Board::new());
    }

    #[test]
    fn lee_bfs_path_matches_naive_on_startpos() {
        assert_bfs_path_matches_naive(&Board::new());
    }

    #[test]
    fn lee_bfs_path_matches_naive_on_random_legal_positions() {
        let mut seed = 0x6a09_e667_f3bc_c909u64;
        let mut next_random = || {
            seed ^= seed >> 12;
            seed ^= seed << 25;
            seed ^= seed >> 27;
            seed.wrapping_mul(0x2545_f491_4f6c_dd1d)
        };

        for target_ply in [4usize, 20, 35] {
            for _ in 0..100 {
                let mut board = Board::new();
                for _ in 0..target_ply {
                    if board.is_terminal().is_some() {
                        break;
                    }
                    let moves = generate_legal_moves(&board);
                    if moves.is_empty() {
                        break;
                    }
                    let mv = moves[(next_random() as usize) % moves.len()];
                    board.apply_move(mv);
                }
                assert_bfs_path_matches_naive(&board);
            }
        }
    }

    #[test]
    fn both_reachable_mask_includes_both_pawns() {
        let board = Board::new();
        let mut scratch = BfsScratch::new();
        let mask = scratch.both_reachable_mask(&board);
        assert_ne!(mask & (1u128 << square_index(0, 4)), 0);
        assert_ne!(mask & (1u128 << square_index(8, 4)), 0);
    }

    #[test]
    fn full_barrier_blocks_p1() {
        let mut board = Board::new();
        for c in 0..8u8 {
            set_wall(&mut board, 6, c, WallOrientation::Horizontal, true);
        }
        assert!(!can_reach_goal(&board, Player::One));
    }
}
