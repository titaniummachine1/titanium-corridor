//! `genmove` entry — MCTS default (Gorisanson-style), minimax optional.

use crate::board::Board;
use crate::greedy::choose_greedy_move;
use crate::mcts::{genmove_algebraic as mcts_algebraic, MctsConfig, DEFAULT_TIME_MS};
use crate::perft::format_move;
use crate::search::{genmove_algebraic as minimax_algebraic, SearchConfig, DEFAULT_MAX_NODES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenmoveEngine {
    Mcts,
    Minimax,
    Greedy,
}

impl Default for GenmoveEngine {
    fn default() -> Self {
        Self::Mcts
    }
}

#[derive(Debug, Clone)]
pub struct GenmoveConfig {
    pub engine: GenmoveEngine,
    pub mcts: MctsConfig,
    pub minimax: SearchConfig,
}

impl Default for GenmoveConfig {
    fn default() -> Self {
        Self {
            engine: GenmoveEngine::Mcts,
            mcts: MctsConfig::default(),
            minimax: SearchConfig {
                time_ms: DEFAULT_TIME_MS,
                max_nodes: DEFAULT_MAX_NODES,
                log: false,
            },
        }
    }
}

pub fn genmove_algebraic(board: &mut Board, config: GenmoveConfig) -> Option<String> {
    match config.engine {
        GenmoveEngine::Mcts => mcts_algebraic(board, config.mcts),
        GenmoveEngine::Minimax => minimax_algebraic(board, config.minimax),
        GenmoveEngine::Greedy => greedy_algebraic(board),
    }
}

fn greedy_algebraic(board: &mut Board) -> Option<String> {
    let mut scratch = crate::path::BfsScratch::new();
    choose_greedy_move(board, &mut scratch).map(format_move)
}

pub use crate::mcts::DEFAULT_UCT as MCTS_DEFAULT_UCT;
pub use crate::mcts::DEFAULT_MAX_SIMULATIONS as MCTS_DEFAULT_MAX_SIMULATIONS;
