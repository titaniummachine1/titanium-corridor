//! wasm-bindgen bindings for the website (GitHub Pages + static hosting).
//!
//! Build (from repo root):
//!   cd site/web && npm run build:wasm

use wasm_bindgen::prelude::*;

use crate::cat::cat_snapshot_json;
use crate::core::board::Board;
use crate::titanium::net::{
    frozen_weights_sha256, install_medium_weights, live_weights_sha256, NET_WEIGHT_BYTE_LEN,
};
use crate::titanium::search::think_result_progress_json;
use crate::titanium::{
    algebraic_to_move_id, move_id_to_algebraic, GameState, ThinkResult, TitaniumParams,
    TitaniumSearch, TITANIUM_NO_MOVE,
};

const ENGINE_VERSION: &str = "titanium-v15";
const WASM_FEATURES: &str = "wasm,embed-tables";

fn titanium_v15_params_from_mode(
    engine_mode: &str,
    movetime_ms: u32,
    max_depth: i32,
) -> TitaniumParams {
    let ti_movegen = engine_mode.contains("-ti") || engine_mode == "ace-v13";
    let eme = engine_mode.contains("pmc");
    TitaniumParams {
        time_ms: (movetime_ms as u64).max(1),
        max_depth: if max_depth > 0 { max_depth } else { 30 },
        threads: 1,
        full: false,
        cat: false,
        ti_movegen,
        log: false,
        eme,
    }
}

fn ace_params_from_mode(
    engine_mode: &str,
    movetime_ms: u32,
    max_depth: i32,
) -> crate::ace::AceParams {
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

fn is_titanium_v15_mode(engine_mode: &str) -> bool {
    engine_mode.starts_with("ace-v13") || engine_mode == "ace-v13"
}

fn build_titanium_search(
    g: GameState,
    params: TitaniumParams,
    engine_label: &str,
) -> TitaniumSearch {
    let mut search = match engine_label {
        "titanium-v15" | "titanium-v14" | "ace-v13-grafted" => *TitaniumSearch::grafted(g, None),
        "titanium-v16" => *TitaniumSearch::grafted_v16(g, None),
        "titanium-v15-frozen" => *TitaniumSearch::grafted_frozen(g, None),
        "titanium-v15-medium" => *TitaniumSearch::grafted_medium(g, None),
        "titanium-v15-no-raceproof" | "ace-v13-grafted-no-raceproof" => {
            *TitaniumSearch::grafted_no_raceproof(g, None)
        }
        "ace-v13" | "ace-v13-ti" => *TitaniumSearch::with_ti_movegen_frozen(g),
        "ace-v13-ti-pure" => *TitaniumSearch::with_ti_movegen_pure(g),
        _ if params.ti_movegen && params.cat => *TitaniumSearch::with_ti_movegen_and_cat(g),
        _ if params.ti_movegen => *TitaniumSearch::with_ti_movegen(g),
        _ if params.cat => *TitaniumSearch::with_cat(g),
        _ => *TitaniumSearch::new(g),
    };
    if params.eme {
        search.enable_eme();
    }
    search
}

fn titanium_genmove_with_progress(
    moves: &str,
    params: TitaniumParams,
    engine_label: &str,
    on_progress: Option<js_sys::Function>,
) -> Option<(String, ThinkResult)> {
    let g = replay_moves(moves).ok()?;
    if g.winner() >= 0 {
        return None;
    }
    let mut search = build_titanium_search(g, params, engine_label);
    search.set_wasm_progress(on_progress.clone());
    let stream = on_progress.is_some();
    let result = search.think(
        params.time_ms,
        params.max_depth,
        params.full,
        stream,
        engine_label,
    );
    if result.mv == TITANIUM_NO_MOVE {
        return None;
    }
    if result.mv == 0 && search.g.winner() >= 0 {
        return None;
    }
    if let Some(f) = on_progress.as_ref() {
        let json = think_result_progress_json(engine_label, &result);
        let _ = f.call1(&JsValue::NULL, &JsValue::from_str(&json));
    }
    Some((move_id_to_algebraic(result.mv), result))
}

fn replay_moves(moves: &str) -> Result<GameState, JsError> {
    let mut g = GameState::new();
    for text in moves.split_whitespace().filter(|s| !s.is_empty()) {
        if g.winner() >= 0 {
            return Err(JsError::new(&format!(
                "illegal replay past terminal: {text}"
            )));
        }
        g.make_move(algebraic_to_move_id(text));
    }
    Ok(g)
}

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn replay_board_from_moves(moves: &str) -> Board {
    let mut board = Board::new();
    for text in moves.split_whitespace().filter(|s| !s.is_empty()) {
        board.apply_algebraic(text);
    }
    board
}

/// CAT v3 heatmap JSON for the website overlay (`catHeatmap.js`).
#[wasm_bindgen]
pub fn cat_snapshot(moves: &str) -> String {
    let mut board = replay_board_from_moves(moves);
    cat_snapshot_json(&mut board)
}

/// JSON build identity for browser debug panel / console.
#[wasm_bindgen]
pub fn wasm_build_identity_json() -> String {
    let git = option_env!("GIT_COMMIT_HASH").unwrap_or("unknown");
    let built_at = option_env!("WASM_BUILD_TIMESTAMP").unwrap_or("unknown");
    format!(
        r#"{{"engine_version":"{ENGINE_VERSION}","git_commit":"{git}","build_timestamp":"{built_at}","features":"{WASM_FEATURES}","weights_live_sha256":"{live}","weights_frozen_sha256":"{frozen}"}}"#,
        live = hex32(&live_weights_sha256()),
        frozen = hex32(&frozen_weights_sha256()),
    )
}

/// Byte length of an NNUE weights blob (`net_weights*.bin`).
#[wasm_bindgen]
pub fn net_weight_byte_len() -> usize {
    NET_WEIGHT_BYTE_LEN
}

/// Install runtime medium-tier weights (`tier` must be `1`).
#[wasm_bindgen]
pub fn install_net_weights(tier: u8, bytes: &[u8]) -> Result<(), JsError> {
    if tier != 1 {
        return Err(JsError::new(
            "install_net_weights: only tier 1 (medium) is supported",
        ));
    }
    install_medium_weights(bytes).map_err(|e| JsError::new(e))
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

    pub fn genmove(
        &self,
        moves: &str,
        movetime_ms: u32,
        max_depth: i32,
        engine_mode: &str,
        on_progress: Option<js_sys::Function>,
    ) -> String {
        let list: Vec<String> = moves
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if is_titanium_v15_mode(engine_mode) {
            let params = titanium_v15_params_from_mode(engine_mode, movetime_ms, max_depth);
            return match titanium_genmove_with_progress(moves, params, engine_mode, on_progress) {
                Some((alg, _)) => alg,
                None => "(none)".to_string(),
            };
        }
        let params = ace_params_from_mode(engine_mode, movetime_ms, max_depth);
        match crate::ace::ace_genmove(&list, params, engine_mode) {
            Some((alg, _)) => alg,
            None => "(none)".to_string(),
        }
    }
}

/// Warm titanium-v15 session — TT / history persist between plies (GitHub Pages Titanium).
#[wasm_bindgen]
pub struct WasmEngine {
    search: TitaniumSearch,
    engine_label: String,
    last_depth: i32,
    last_nodes: u64,
    last_stop_reason: &'static str,
}

#[wasm_bindgen]
impl WasmEngine {
    /// `tier`: 0 = frozen (Easy), 1 = medium, 2 = hard (v15 live),
    /// 3/4/5 = v16 CAT LMR with ceiling 500 / 800 / 1000 cm.
    #[wasm_bindgen(constructor)]
    pub fn new(tier: u8) -> WasmEngine {
        let g = GameState::new();
        let (search, engine_label) = match tier {
            0 => (
                *TitaniumSearch::grafted_frozen(g, None),
                "titanium-v15-frozen".to_string(),
            ),
            1 => (
                *TitaniumSearch::grafted_medium(g, None),
                "titanium-v15-medium".to_string(),
            ),
            3 | 4 | 5 => {
                let ceiling = match tier {
                    3 => 500,
                    5 => 1000,
                    _ => 800,
                };
                (
                    *TitaniumSearch::grafted_v16_with_ceiling(g, None, ceiling),
                    "titanium-v16".to_string(),
                )
            }
            _ => (
                *TitaniumSearch::grafted(g, None),
                "titanium-v15".to_string(),
            ),
        };
        WasmEngine {
            search,
            engine_label,
            last_depth: 0,
            last_nodes: 0,
            last_stop_reason: "none",
        }
    }

    pub fn reset(&mut self) {
        self.search.set_position(GameState::new());
    }

    pub fn position(&mut self, moves: &str) -> Result<usize, JsError> {
        let g = replay_moves(moves)?;
        let n = moves.split_whitespace().filter(|s| !s.is_empty()).count();
        self.search.set_position(g);
        Ok(n)
    }

    pub fn make_move(&mut self, mv: &str) -> bool {
        if self.search.g.winner() >= 0 {
            return false;
        }
        self.search.apply_move(algebraic_to_move_id(mv));
        true
    }

    pub fn go(
        &mut self,
        movetime_ms: u32,
        _max_nodes: u32,
        on_progress: Option<js_sys::Function>,
    ) -> String {
        self.go_with_profile(movetime_ms, _max_nodes, 0, 0, 0, on_progress)
    }

    /// `worker_id`: only worker 0 streams progress callbacks (matches multi-worker client).
    pub fn go_with_profile(
        &mut self,
        movetime_ms: u32,
        _max_nodes: u32,
        worker_id: u32,
        _late_wall_skip_pct: u32,
        _lmr_bias: i32,
        on_progress: Option<js_sys::Function>,
    ) -> String {
        self.search.set_cat_lmr_worker_profile(worker_id as usize);
        let stream = on_progress.is_some() && worker_id == 0;
        self.search
            .set_wasm_progress(if stream { on_progress.clone() } else { None });
        if self.search.g.winner() >= 0 {
            self.last_depth = 0;
            self.last_nodes = 0;
            self.last_stop_reason = "terminal";
            return "(none)".to_string();
        }
        let result = self.search.think(
            (movetime_ms as u64).max(1),
            30,
            false,
            stream,
            &self.engine_label,
        );
        self.search.set_wasm_progress(None);
        self.last_depth = result.depth;
        self.last_nodes = result.nodes;
        self.last_stop_reason = result.stop_reason;
        if stream {
            if let Some(f) = on_progress.as_ref() {
                let json = think_result_progress_json(&self.engine_label, &result);
                let _ = f.call1(&JsValue::NULL, &JsValue::from_str(&json));
            }
        }
        if result.mv == TITANIUM_NO_MOVE {
            "(none)".to_string()
        } else {
            move_id_to_algebraic(result.mv)
        }
    }

    pub fn last_search_depth(&self) -> i32 {
        self.last_depth
    }

    pub fn last_search_nodes(&self) -> u64 {
        self.last_nodes
    }

    pub fn last_stop_reason(&self) -> String {
        self.last_stop_reason.to_string()
    }

    pub fn engine_mode(&self) -> String {
        self.engine_label.clone()
    }

    pub fn legal_moves(&self) -> String {
        String::new()
    }

    pub fn winner(&self) -> i32 {
        let w = self.search.g.winner();
        if w < 0 {
            -1
        } else {
            w
        }
    }
}
