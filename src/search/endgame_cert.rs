//! Bounded endgame win/loss certificates used at alpha-beta leaves.
//!
//! This module owns the proof budget, cache, and score band.  Keeping those
//! details together makes the leaf evaluator responsible only for deciding
//! when to ask for a solved outcome.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cat::constants::DIST_PENALTY;
use crate::core::board::{Board, Player};
use crate::path::BfsScratch;

/// Proven outcome must dominate static eval but remain below mate scores.
const CERT_WIN: i32 = 15_000;
/// Static-eval band retained to order moves inside a proven outcome class.
const CERT_BAND: i32 = 4_000;
/// Only close races can overturn the static distance evaluation.
const CERT_TEMPO_MARGIN: i32 = 2;

/// Count of endgame certificate proofs returned by this process.
///
/// This is observability for strength matches only; it never affects search.
pub static CERT_PROOFS: AtomicU64 = AtomicU64::new(0);

/// Endgame guaranteed-win/loss proof oracle (v13 `certify_win` via
/// `titanium::cert_bridge`). Caches verdicts by `(hash, side)` and bounds total
/// certify attempts per search so the cost stays negligible.
pub(crate) struct EndgameCert {
    enabled: bool,
    /// Per-attempt certify node budget.
    budget: u64,
    /// Max certify attempts per whole search; cache hits are free and uncapped.
    cap: u32,
    calls: u32,
    /// `(board hash, side)` -> proven side (`0`/`1`) or `2` when unproven.
    cache: HashMap<(u64, u8), u8>,
}

impl EndgameCert {
    pub(crate) fn new(override_: Option<bool>) -> Self {
        // ON by default: path-aware classifier measured +85 Elo at 2s/move.
        // Set TITANIUM_ENDGAME_CERT=0 to disable, or pass Some(false) via SearchConfig.
        let enabled = override_.unwrap_or_else(|| {
            std::env::var("TITANIUM_ENDGAME_CERT")
                .map(|v| v != "0")
                .unwrap_or(true)
        });
        let budget = std::env::var("TITANIUM_CERT_BUDGET")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1200);
        let cap = std::env::var("TITANIUM_CERT_CAP")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(64);
        Self {
            enabled,
            budget,
            cap,
            calls: 0,
            cache: HashMap::new(),
        }
    }

    /// Return a score only when the certificate can prove the leaf outcome.
    /// `None` leaves ordinary static evaluation unchanged.
    pub(crate) fn score_for(
        &mut self,
        board: &Board,
        stm: Player,
        static_eval: i32,
        bfs: &mut BfsScratch,
    ) -> Option<i32> {
        if !self.enabled {
            return None;
        }

        // Outcome class dominates; static eval orders within the class so a
        // proven win converts cleanly and a proven loss still resists well.
        let band = static_eval.clamp(-CERT_BAND, CERT_BAND);
        let win = CERT_WIN + band;
        let loss = -CERT_WIN + band;

        // With no walls this is a pawn race. The cheap classifier resolves the
        // ordinary cases; only the volatile overlap case needs minimax proof.
        if board.walls_remaining[0] == 0 && board.walls_remaining[1] == 0 {
            use crate::titanium::cert_bridge::{hands_empty_race, RaceVerdict};
            let mut game = crate::titanium::cert_bridge::titanium_game_from_board(board);
            match hands_empty_race(&game) {
                RaceVerdict::Win => {
                    CERT_PROOFS.fetch_add(1, Ordering::Relaxed);
                    return Some(win);
                }
                RaceVerdict::Loss => {
                    CERT_PROOFS.fetch_add(1, Ordering::Relaxed);
                    return Some(loss);
                }
                RaceVerdict::NeedsProof => {
                    match crate::titanium::cert_bridge::race_minimax(&mut game) {
                        crate::titanium::cert_bridge::RaceProof::Win => {
                            CERT_PROOFS.fetch_add(1, Ordering::Relaxed);
                            return Some(win);
                        }
                        crate::titanium::cert_bridge::RaceProof::Loss => {
                            CERT_PROOFS.fetch_add(1, Ordering::Relaxed);
                            return Some(loss);
                        }
                        crate::titanium::cert_bridge::RaceProof::Unknown => return None,
                    }
                }
            }
        }

        // Experimental wall-ignorance corridor certificate (feature-gated).
        if board.walls_remaining[0] + board.walls_remaining[1] > 0 {
            if let Some(verdict) =
                crate::titanium::wall_ignore_cert::try_wall_ignore_cert_board(board, false)
            {
                CERT_PROOFS.fetch_add(1, Ordering::Relaxed);
                return Some(crate::titanium::wall_ignore_cert::cert_score_from_player(
                    &verdict, stm,
                ));
            }
        }

        let our_dist = bfs.shortest_distance(board, stm).unwrap_or(DIST_PENALTY);
        let opp_dist = bfs
            .shortest_distance(board, stm.opposite())
            .unwrap_or(DIST_PENALTY);
        if (our_dist as i32 - opp_dist as i32).abs() > CERT_TEMPO_MARGIN {
            return None;
        }
        if board.walls_remaining[stm as usize] > 2 || static_eval.abs() >= 3_000 {
            return None;
        }

        let key = (board.hash, stm as u8);
        let proven = if let Some(&cached) = self.cache.get(&key) {
            cached
        } else {
            if self.calls >= self.cap {
                return None;
            }
            self.calls += 1;
            let verdict = crate::titanium::cert_bridge::certify_board(board, self.budget, 0, None);
            let code = match verdict {
                Some(Player::One) => 0,
                Some(Player::Two) => 1,
                None => 2,
            };
            self.cache.insert(key, code);
            code
        };

        match proven {
            2 => None,
            side if side == stm as u8 => {
                CERT_PROOFS.fetch_add(1, Ordering::Relaxed);
                Some(win)
            }
            _ => {
                CERT_PROOFS.fetch_add(1, Ordering::Relaxed);
                Some(loss)
            }
        }
    }
}
