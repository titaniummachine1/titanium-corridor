//! Exact BFS wavefront layers, distance fields, and Lee path reconstruction.

// Exact BFS wavefront layers and Lee path reconstruction.
use crate::core::board::Player;
use crate::pathfinding::bff::expand_frontier;
use crate::pathfinding::masks::DirMasks;
use crate::util::grid::{flood_bit_sq, goal_row, square_index, FLOOD_PLAYABLE, FLOOD_SQ_BY_BIT};

/// Max BFS layers we record — board diameter is bounded by the 81 playable cells.
pub const MAX_DIST_LAYERS: usize = 81;

/// Per-layer frontier masks from a parallel flood. `masks[d]` is the set of cells
/// reached at BFS distance exactly `d`; `depth` is how many layers are populated.
///
/// This is the "distance-indexed" representation: instead of scattering a scalar
/// `dist[sq]` array, the flood keeps each wavefront as a u128 mask. Consumers that
/// want *sets at a distance* (corridor bands, reachability) read masks directly;
/// `to_scalar_field` materializes the dense array only when random lookup is needed.
#[derive(Clone)]
pub struct DistLayers {
    pub masks: [u128; MAX_DIST_LAYERS],
    pub depth: usize,
}

impl Default for DistLayers {
    fn default() -> Self {
        Self {
            masks: [0u128; MAX_DIST_LAYERS],
            depth: 0,
        }
    }
}

impl DistLayers {
    /// Reconstruct a dense `dist[sq]` field (`u8::MAX` = unreachable) from the
    /// layer masks — used by the parity test and by consumers needing scalar reads.
    pub fn to_scalar_field(&self, out: &mut [u8; 81]) {
        out.fill(u8::MAX);
        for d in 0..self.depth {
            let mut bits = self.masks[d] & FLOOD_PLAYABLE;
            while bits != 0 {
                let fb = bits.trailing_zeros();
                bits &= bits - 1;
                let sq = FLOOD_SQ_BY_BIT[fb as usize];
                out[sq as usize] = d as u8;
            }
        }
    }

    /// Fill one deterministic next-step map from these inverse BFS layers.
    ///
    /// A square in layer `d` points to an adjacent square in layer `d - 1`.
    /// Layer zero is the BFS seed (normally a goal row) and has no next step.
    /// This is standard Lee wavefront backtracking without a parent queue.
    pub fn fill_bfs_next_steps(&self, masks: DirMasks, next_out: &mut [u8; 81]) {
        next_out.fill(u8::MAX);

        for depth in 1..self.depth {
            let previous = self.masks[depth - 1];
            let mut current_layer = self.masks[depth] & FLOOD_PLAYABLE;
            while current_layer != 0 {
                let current_bit_index = current_layer.trailing_zeros();
                let current = 1u128 << current_bit_index;
                current_layer &= current_layer - 1;

                let candidates = expand_frontier(current, masks) & previous;
                debug_assert_ne!(candidates, 0, "BFS layer has no predecessor");
                if candidates == 0 {
                    continue;
                }

                let previous_bit_index = candidates.trailing_zeros();
                let current_sq = FLOOD_SQ_BY_BIT[current_bit_index as usize];
                let previous_sq = FLOOD_SQ_BY_BIT[previous_bit_index as usize];
                next_out[current_sq as usize] = previous_sq;
            }
        }
    }

    /// Destructively reconstruct one shortest path to `target`.
    ///
    /// The layers must have been stopped at the first BFS wavefront that touches
    /// `target`. Starting at that target, each pop exposes the immediately
    /// preceding Lee BFS wavefront. The chosen adjacent bit is therefore always
    /// one step closer to the seed. Consumed layers are cleared as we walk back.
    pub fn pop_shortest_path_to(
        &mut self,
        target: u128,
        masks: DirMasks,
        path_out: &mut [u8; 81],
    ) -> Option<usize> {
        if self.depth == 0 {
            return None;
        }

        let path_len = self.depth;
        let mut candidates = self.masks[self.depth - 1] & target;
        if candidates == 0 {
            return None;
        }

        let mut current = candidates & candidates.wrapping_neg();
        let current_bit_index = current.trailing_zeros();
        path_out[path_len - 1] = FLOOD_SQ_BY_BIT[current_bit_index as usize];

        while self.depth > 1 {
            self.depth -= 1;
            self.masks[self.depth] = 0;

            candidates = expand_frontier(current, masks) & self.masks[self.depth - 1];
            debug_assert_ne!(candidates, 0, "BFS layer has no predecessor");
            if candidates == 0 {
                self.depth = 0;
                return None;
            }

            current = candidates & candidates.wrapping_neg();
            let bit_index = current.trailing_zeros();
            path_out[self.depth - 1] = FLOOD_SQ_BY_BIT[bit_index as usize];
        }

        self.masks[0] = 0;
        self.depth = 0;
        Some(path_len)
    }
}

/// Parallel binary flood that records each wavefront as a layer mask (no per-cell
/// scatter). `seed` is the starting frontier (single cell for pawn floods, the
/// goal row for inverse floods). Layer `d`'s mask holds cells at BFS distance `d`.
fn flood_layers(seed: u128, masks: DirMasks, out: &mut DistLayers) {
    let mut reached = seed & FLOOD_PLAYABLE;
    let mut frontier = reached;
    out.masks[0] = frontier;
    let mut depth = 1usize;
    while frontier != 0 && depth < MAX_DIST_LAYERS {
        let new = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        if new == 0 {
            break;
        }
        out.masks[depth] = new;
        depth += 1;
        reached |= new;
        frontier = new;
    }
    out.depth = depth;
}

/// Binary Flood Fill (BFF) recorded as exact BFS wavefront layers. Stops at the
/// first layer touching `target`, which is sufficient for shortest-path recovery.
fn flood_layers_until_target(
    seed: u128,
    target: u128,
    masks: DirMasks,
    out: &mut DistLayers,
) -> bool {
    let mut reached = seed & FLOOD_PLAYABLE;
    let mut frontier = reached;
    out.masks[0] = frontier;
    let mut depth = 1usize;

    if frontier & target != 0 {
        out.depth = depth;
        return true;
    }

    while frontier != 0 && depth < MAX_DIST_LAYERS {
        frontier = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        if frontier == 0 {
            break;
        }
        out.masks[depth] = frontier;
        depth += 1;
        reached |= frontier;
        if frontier & target != 0 {
            out.depth = depth;
            return true;
        }
    }

    out.depth = depth;
    false
}

/// Forward flood: layer masks of distance from `start`.
pub fn fill_dist_layers_from_sq(start: u8, masks: DirMasks, out: &mut DistLayers) {
    flood_layers(flood_bit_sq(start), masks, out);
}

/// Forward BFS from `start`, retaining only the exact wavefronts needed to reach
/// `player`'s goal row. Returns false when the goal is unreachable.
pub fn fill_bfs_layers_until_goal(
    start: u8,
    player: Player,
    masks: DirMasks,
    out: &mut DistLayers,
) -> bool {
    flood_layers_until_target(flood_bit_sq(start), goal_mask(player), masks, out)
}

/// Inverse flood: layer masks of distance to any goal-row cell for `player`.
pub fn fill_dist_layers_to_goal_row(player: Player, masks: DirMasks, out: &mut DistLayers) {
    let grow = goal_row(player);
    let mut seed = 0u128;
    for c in 0..9u8 {
        seed |= flood_bit_sq(square_index(grow, c));
    }
    flood_layers(seed, masks, out);
}

#[inline]
fn goal_mask(player: Player) -> u128 {
    let row = goal_row(player);
    let mut mask = 0u128;
    for col in 0..9u8 {
        mask |= flood_bit_sq(square_index(row, col));
    }
    mask
}

/// Fill `dist_from[sq]` with BFS distance from `start`. Unreachable → `u8::MAX`.
pub fn fill_dist_from_sq(start: u8, masks: DirMasks, dist_from: &mut [u8; 81]) -> u128 {
    dist_from.fill(u8::MAX);
    dist_from[start as usize] = 0;
    let mut reached = flood_bit_sq(start);
    let mut frontier = reached;
    let mut layer = 0u8;
    while frontier != 0 {
        layer += 1;
        let new = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        if new == 0 {
            break;
        }
        let mut bits = new;
        while bits != 0 {
            let fb = bits.trailing_zeros();
            bits &= bits - 1;
            let sq = FLOOD_SQ_BY_BIT[fb as usize];
            dist_from[sq as usize] = layer;
        }
        reached |= new;
        frontier = new;
    }
    reached
}

/// Fill `dist_to[sq]` with BFS distance to any goal-row cell for `player`.
pub fn fill_dist_to_goal_row(player: Player, masks: DirMasks, dist_to: &mut [u8; 81]) {
    let grow = goal_row(player);
    dist_to.fill(u8::MAX);

    let mut reached = 0u128;
    for c in 0..9u8 {
        let sq = square_index(grow, c);
        dist_to[sq as usize] = 0;
        reached |= flood_bit_sq(sq);
    }

    let mut frontier = reached;
    let mut layer = 0u8;
    while frontier != 0 {
        layer += 1;
        let new = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        if new == 0 {
            break;
        }
        let mut bits = new;
        while bits != 0 {
            let fb = bits.trailing_zeros();
            bits &= bits - 1;
            let sq = FLOOD_SQ_BY_BIT[fb as usize];
            dist_to[sq as usize] = layer;
        }
        reached |= new;
        frontier = new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::board::Board;

    /// The layered flood, reconstructed to a dense array, must equal the scalar
    /// BFS field square-for-square — the oracle the bitmask refactor is held to.
    fn assert_layers_match_scalar(board: &Board) {
        let masks = DirMasks::from_board(board);
        let mut layers = DistLayers::default();
        let mut from_layers = [0u8; 81];
        let mut from_scalar = [0u8; 81];

        // Forward floods from each pawn.
        for player in [Player::One, Player::Two] {
            let (r, c) = board.pawn(player);
            let start = square_index(r, c);
            fill_dist_layers_from_sq(start, masks, &mut layers);
            layers.to_scalar_field(&mut from_layers);
            fill_dist_from_sq(start, masks, &mut from_scalar);
            assert_eq!(
                from_layers, from_scalar,
                "forward field mismatch for {player:?}"
            );

            // Inverse floods to each goal row.
            fill_dist_layers_to_goal_row(player, masks, &mut layers);
            layers.to_scalar_field(&mut from_layers);
            fill_dist_to_goal_row(player, masks, &mut from_scalar);
            assert_eq!(
                from_layers, from_scalar,
                "inverse field mismatch for {player:?}"
            );
        }
    }

    #[test]
    fn layered_flood_matches_scalar_startpos() {
        assert_layers_match_scalar(&Board::new());
    }

    #[test]
    fn layered_flood_matches_scalar_with_walls() {
        let mut board = Board::new();
        for mv in ["e2", "e8", "e3", "e7", "d3h", "f5v", "c2h", "g6h"] {
            board.apply_algebraic(mv);
        }
        assert_layers_match_scalar(&board);
    }

    /// Dump forward/inverse/delta fields as CSV grids for visual inspection.
    /// Run with: cargo test -p titanium dump_fields_for_viz -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_fields_for_viz() {
        let mut board = Board::new();
        for mv in ["e2", "e8", "e3", "e7", "d3h", "f5v", "c2h", "g6h"] {
            board.apply_algebraic(mv);
        }
        let masks = DirMasks::from_board(&board);
        let player = Player::One;
        let (r, c) = board.pawn(player);
        let start = square_index(r, c);

        let mut layers = DistLayers::default();
        let mut fwd = [0u8; 81];
        let mut inv = [0u8; 81];
        fill_dist_layers_from_sq(start, masks, &mut layers);
        layers.to_scalar_field(&mut fwd);
        fill_dist_layers_to_goal_row(player, masks, &mut layers);
        layers.to_scalar_field(&mut inv);

        let s = inv[start as usize];
        println!("PAWN_SQ={start} SHORTEST={s}");
        // Print rows 8..0 (player One moves toward row 8 goal).
        let dump = |name: &str, f: &[u8; 81]| {
            println!("--{name}--");
            for row in (0..9u8).rev() {
                let cells: Vec<String> = (0..9u8)
                    .map(|col| {
                        let v = f[square_index(row, col) as usize];
                        if v == u8::MAX {
                            "X".into()
                        } else {
                            v.to_string()
                        }
                    })
                    .collect();
                println!("{}", cells.join(","));
            }
        };
        dump("FORWARD_FROM_PAWN", &fwd);
        dump("INVERSE_TO_GOAL", &inv);
        // delta = fwd + inv - s (off-path excess); X where unreachable.
        let mut delta = [u8::MAX; 81];
        for sq in 0..81 {
            if fwd[sq] != u8::MAX && inv[sq] != u8::MAX {
                delta[sq] = (fwd[sq] + inv[sq]).saturating_sub(s);
            }
        }
        dump("DELTA_OFF_PATH", &delta);
    }
}
