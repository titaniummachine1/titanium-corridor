//! Board-facing BFS queries shared by engine systems.

// Board-facing BFS queries belong here; layer mechanics live in `layers`.
pub mod layers;

use crate::core::board::{Board, Player};
use crate::pathfinding::bff::{
    flood_fill_flood_bits, flood_to_goal, flood_to_goal_seeded, flood_to_goal_with_depth,
    goal_square_mask,
};
use crate::pathfinding::bfs::layers::{
    fill_bfs_layers_until_goal, fill_dist_layers_to_goal_row, DistLayers,
};
use crate::pathfinding::masks::DirMasks;
use crate::util::grid::{pack_flood_mask, square_index};

/// Reused BFS workspace — pass through perft and move-generation hot loops.
#[derive(Clone)]
pub struct BfsScratch {
    dist_from_pawn: [u8; 81],
    dist_to_goal: [u8; 81],
    /// Cached `DirMasks` for the current board hash — one build per movegen node.
    masks_hash: u64,
    masks: DirMasks,
}

impl Default for BfsScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl BfsScratch {
    pub fn new() -> Self {
        Self {
            dist_from_pawn: [0; 81],
            dist_to_goal: [0; 81],
            masks_hash: 0,
            masks: DirMasks::default(),
        }
    }

    /// Direction masks for the current position — rebuilt only when `board.hash` changes.
    #[inline]
    pub fn dir_masks(&mut self, board: &Board) -> DirMasks {
        if self.masks_hash != board.hash {
            self.masks_hash = board.hash;
            self.masks = DirMasks::from_board(board);
        }
        self.masks
    }

    /// Call after in-place wall trials — `board.hash` may match a stale cache entry.
    #[inline]
    pub fn invalidate_dir_masks(&mut self) {
        self.masks_hash = !0;
    }

    pub(crate) fn dist_scratch_mut(&mut self) -> (&mut [u8; 81], &mut [u8; 81]) {
        (&mut self.dist_from_pawn, &mut self.dist_to_goal)
    }

    #[inline]
    pub fn can_reach_goal(&mut self, board: &Board, player: Player) -> bool {
        let masks = self.dir_masks(board);
        let (sr, sc) = board.pawn(player);
        let start = square_index(sr, sc);
        flood_to_goal(start, masks, goal_square_mask(player)).0
    }

    #[inline]
    pub fn both_players_reach_goals(&mut self, board: &Board) -> bool {
        both_players_reach_goals_with_masks(board, self.dir_masks(board))
    }

    pub fn fill_reachable(&mut self, board: &Board, player: Player, mask: &mut u128) {
        let masks = self.dir_masks(board);
        let (sr, sc) = board.pawn(player);
        let start = square_index(sr, sc);
        *mask |= pack_flood_mask(flood_fill_flood_bits(start, masks));
    }

    pub fn both_reachable_mask(&mut self, board: &Board) -> u128 {
        let mut mask = 0u128;
        self.fill_reachable(board, Player::One, &mut mask);
        self.fill_reachable(board, Player::Two, &mut mask);
        mask
    }

    pub fn shortest_distance(&mut self, board: &Board, player: Player) -> Option<u8> {
        let masks = self.dir_masks(board);
        let (sr, sc) = board.pawn(player);
        let start = square_index(sr, sc);
        let (reached_goal, _, depth) =
            flood_to_goal_with_depth(start, masks, goal_square_mask(player));
        reached_goal.then_some(depth)
    }

    /// Reconstruct one deterministic shortest path using exact BFS wavefronts.
    ///
    /// The Binary Flood Fill records one frontier mask per distance. Standard
    /// Lee BFS backtracking then pops those layers from the reached goal toward
    /// the pawn, without a queue, parent table, or scalar distance field.
    pub fn shortest_path(
        &mut self,
        board: &Board,
        player: Player,
        path_out: &mut [u8; 81],
    ) -> Option<usize> {
        let masks = self.dir_masks(board);
        let (row, col) = board.pawn(player);
        let start = square_index(row, col);
        let mut layers = DistLayers::default();

        if !fill_bfs_layers_until_goal(start, player, masks, &mut layers) {
            return None;
        }

        layers.pop_shortest_path_to(goal_square_mask(player), masks, path_out)
    }

    pub fn fill_next_toward_goal(
        &mut self,
        board: &Board,
        player: Player,
        next_out: &mut [u8; 81],
    ) {
        let masks = self.dir_masks(board);
        let mut layers = DistLayers::default();
        fill_dist_layers_to_goal_row(player, masks, &mut layers);
        layers.fill_bfs_next_steps(masks, next_out);
    }
}

/// Both players can reach their goal row — uses caller-supplied masks (wall trials).
#[inline]
pub fn both_players_reach_goals_with_masks(board: &Board, masks: DirMasks) -> bool {
    let (r1, c1) = board.pawn(Player::One);
    let start1 = square_index(r1, c1);
    let goal1 = goal_square_mask(Player::One);
    let (ok1, comp1) = flood_to_goal(start1, masks, goal1);
    if !ok1 {
        return false;
    }

    let (r2, c2) = board.pawn(Player::Two);
    let start2 = square_index(r2, c2);
    let goal2 = goal_square_mask(Player::Two);

    // `comp1` is a *partial* component (P1's flood exits early at its goal), so
    // "start2 ∈ comp1 ⇒ answer is comp1 ∩ goal2" was a false negative whenever
    // P2 needed cells beyond P1's early exit. Seeded flood annexes comp1 on
    // contact and keeps expanding instead.
    flood_to_goal_seeded(start2, comp1, masks, goal2)
}

#[inline]
pub fn can_reach_goal(board: &Board, player: Player) -> bool {
    BfsScratch::new().can_reach_goal(board, player)
}

pub fn shortest_distance(board: &Board, player: Player) -> Option<u8> {
    BfsScratch::new().shortest_distance(board, player)
}

pub fn shortest_path(board: &Board, player: Player, path_out: &mut [u8; 81]) -> Option<usize> {
    BfsScratch::new().shortest_path(board, player, path_out)
}

#[inline]
pub fn both_players_reach_goals(board: &Board) -> bool {
    BfsScratch::new().both_players_reach_goals(board)
}
