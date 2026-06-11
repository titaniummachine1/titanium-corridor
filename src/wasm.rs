//! wasm-bindgen bindings for the website game (Phase 1).
//!
//! Build:
//!   wasm-pack build --release --no-default-features --features wasm
//! or
//!   cargo build --release --lib --target wasm32-unknown-unknown \
//!       --no-default-features --features wasm
//!
//! JS usage:
//!   const e = new WasmEngine();
//!   e.position("e2 e8");          // algebraic moves from startpos
//!   e.make_move("e3");
//!   const best = e.go(1000, 0);   // movetime ms, max_nodes (0 = default)
//!   const moves = e.legal_moves(); // space-separated

use wasm_bindgen::prelude::*;

use crate::movegen::{generate_legal_moves_slice, MAX_LEGAL_MOVES};
use crate::core::board::Move;
use crate::search::alphabeta::{run_search, SearchConfig, DEFAULT_MAX_ID_DEPTH, DEFAULT_MAX_NODES};
use crate::search::session::GameSearchSession;
use crate::util::perft::format_move;

#[wasm_bindgen]
pub struct WasmEngine {
    session: GameSearchSession,
}

#[wasm_bindgen]
impl WasmEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmEngine {
        WasmEngine {
            session: GameSearchSession::new(),
        }
    }

    /// Reset to startpos (clears TT/killers/history).
    pub fn reset(&mut self) {
        self.session.reset();
    }

    /// Set position from startpos via space-separated algebraic moves.
    /// Returns number of moves applied, or throws on illegal move.
    pub fn position(&mut self, moves: &str) -> Result<usize, JsError> {
        let list: Vec<String> = moves.split_whitespace().map(|s| s.to_string()).collect();
        self.session
            .set_position(&list)
            .map_err(|e| JsError::new(&e))
    }

    /// Apply one algebraic move. Returns false if illegal/terminal.
    pub fn make_move(&mut self, mv: &str) -> bool {
        self.session.apply_algebraic(mv)
    }

    /// Search; returns best move in algebraic notation, or "(none)".
    /// `max_nodes = 0` → default node cap.
    pub fn go(&mut self, movetime_ms: u32, max_nodes: u32) -> String {
        if self.session.board.is_terminal().is_some() {
            return "(none)".to_string();
        }
        let config = SearchConfig {
            time_ms: (movetime_ms as u64).max(1),
            max_nodes: if max_nodes == 0 {
                DEFAULT_MAX_NODES
            } else {
                max_nodes as u64
            },
            log: false,
            book_hint: None,
            max_id_depth: DEFAULT_MAX_ID_DEPTH,
        };
        match run_search(&mut self.session, config) {
            Some(report) => format_move(report.best_move),
            None => "(none)".to_string(),
        }
    }

    /// Space-separated legal moves for the side to move.
    pub fn legal_moves(&mut self) -> String {
        let mut buf = [Move::Pawn { row: 0, col: 0 }; MAX_LEGAL_MOVES];
        let mut session_bfs = crate::path::BfsScratch::default();
        let n = generate_legal_moves_slice(&mut self.session.board, &mut buf, &mut session_bfs);
        buf[..n]
            .iter()
            .map(|&m| format_move(m))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 0 = player to move wins not decided; 1/2 = winner; encoded as i32: -1 none, 0, 1.
    pub fn winner(&self) -> i32 {
        match self.session.board.is_terminal() {
            None => -1,
            Some(p) => p as i32,
        }
    }
}
