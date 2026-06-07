//! Titanium Engine — Quoridor search core.
//!
//! Fundamentals: Zobrist hash, make/unmake, TT, iterative deepening perft.
//! Layout: `SharedState` (TT) + `WorkerContext` (per-thread scratch) — Lazy SMP ready.
//! Next: αβ search on the same `Engine` entry point.

pub mod board;
pub mod context;
pub mod engine;
pub mod grid;
pub mod moves;
pub mod path;
pub mod perft;
pub mod tt;
pub mod zobrist;

pub use board::{Board, Column, Move, Player, Row, Undo, WallOrientation};
pub use moves::{
    generate_legal_moves, generate_legal_moves_into, generate_legal_moves_slice, MAX_LEGAL_MOVES,
};
pub use path::{both_players_reach_goals, can_reach_goal, shortest_distance, BfsScratch};
pub use context::{EngineLimits, SharedState, ThreadBenchResult, WorkerContext};
pub use engine::Engine;
pub use perft::{
    format_move, perft, perft_divide, perft_fast, perft_fast_ctx, perft_iterative,
    perft_naive, perft_parallel_root, PerftContext, PERFT3_STARTPOS,
};
pub use tt::TranspositionTable;
