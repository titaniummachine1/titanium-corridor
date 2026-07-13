//! Reachability — bitwise (bitboard) flood fill and `BfsScratch` (no CAT logic here; see `cat`).
//!
//! `parallel::bff_*` = binary flood fill path-to-goal helpers for wall-legality trials.

pub mod bfs;
pub mod distance;
pub mod flood;
pub mod masks;
pub mod parallel;

pub use bfs::{
    both_players_reach_goals, both_players_reach_goals_with_masks, can_reach_goal,
    shortest_distance, BfsScratch,
};
pub use masks::DirMasks;
pub use parallel::{
    bff_ks_to_goal, bff_ks_to_goal_cached, bff_ks_wall_legal, bff_to_goal, bff_to_goal_cached,
    bff_wall_legal, bff_wall_legal_board, wall_delta, WallGrids,
};

#[cfg(test)]
mod tests;
