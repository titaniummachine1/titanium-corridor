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

fn acev13_params_from_mode(
    engine_mode: &str,
    movetime_ms: u32,
    max_depth: i32,
) -> crate::acev13::AceParams {
    let ti_movegen = engine_mode.contains("-ti");
    let eme = engine_mode.contains("pmc");
    crate::acev13::AceParams {
        time_ms: (movetime_ms as u64).max(1),
        max_depth: if max_depth > 0 { max_depth } else { 30 },
        full: false,
        cat: false,
        ti_movegen,
        log: false,
        eme,
    }
}

fn ace_params_from_mode(engine_mode: &str, movetime_ms: u32, max_depth: i32) -> crate::ace::AceParams {
    let ti_movegen = engine_mode.contains("-ti");
    let eme = engine_mode.contains("pmc");
    crate::ace::AceParams {
        time_ms: (movetime_ms as u64).max(1),
        max_depth: if max_depth > 0 { max_depth } else { 30 },
        full: false,
        cat: false,
        ti_movegen,
        log: false,
        eme,
    }
}

fn is_acev13_mode(engine_mode: &str) -> bool {
    engine_mode.starts_with("ace-v13") || engine_mode == "ace-v13"
}

/// ACE Rust port in WASM — one-shot genmove from a move list (GitHub Pages; no native binary).
#[wasm_bindgen]
pub struct WasmAceEngine;

#[wasm_bindgen]
impl WasmAceEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmAceEngine {
        WasmAceEngine
    }

    /// Space-separated algebraic moves from startpos; returns best move or "(none)".
    pub fn genmove(
        &self,
        moves: &str,
        movetime_ms: u32,
        max_depth: i32,
        engine_mode: &str,
    ) -> String {
        let list: Vec<String> = moves
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        let result = if is_acev13_mode(engine_mode) {
            let params = acev13_params_from_mode(engine_mode, movetime_ms, max_depth);
            crate::acev13::ace_genmove(&list, params, engine_mode).map(|(alg, _)| alg)
        } else {
            let params = ace_params_from_mode(engine_mode, movetime_ms, max_depth);
            crate::ace::ace_genmove(&list, params, engine_mode).map(|(alg, _)| alg)
        };
        match result {
            Some(alg) => alg,
            None => "(none)".to_string(),
        }
    }
}

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
