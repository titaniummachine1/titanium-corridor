//! Checkpoint 01 — board + grid + path only.

pub mod board;
pub mod grid;
pub mod path;

pub use board::{Board, Column, Move, Player, Row, WallOrientation};
pub use path::{both_players_reach_goals, can_reach_goal, shortest_distance};
