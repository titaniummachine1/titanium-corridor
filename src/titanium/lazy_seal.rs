//! Node-local lazy-wall seal checks — topology masks + deferred L3 trial context.
//!
//! **Do not** cache topology on `wall_stamp` alone: it counts placed walls, so
//! different layouts can share the same stamp (see `wall_stamp_is_count_not_key`).

use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::board::{Board, WallOrientation};
use crate::movegen::wall_masks::{wall_needs_flood_h_from_bits, wall_needs_flood_v_from_bits};
use crate::path::parallel::{pawn_bit, pbff_wall_legal, wall_delta, WallGrids};
use crate::titanium::game::GameState;
use crate::util::clock::Instant;

/// Counters for lazy-seal A/B (reset in tests via [`reset_lazy_seal_stats`]).
#[derive(Default, Debug)]
pub struct LazySealStats {
    pub nodes_prepared: AtomicU64,
    pub nodes_with_wall_searched: AtomicU64,
    pub nodes_with_risky_wall: AtomicU64,
    pub heavy_contexts_initialized: AtomicU64,
    pub heavy_contexts_used_once: AtomicU64,
    pub walls_examined: AtomicU64,
    pub topo_safe_walls: AtomicU64,
    pub risky_walls: AtomicU64,
    pub illegal_risky_walls: AtomicU64,
    pub heavy_path_checks: AtomicU64,
    pub topo_mask_computes: AtomicU64,
    pub pack_wall_array_ops: AtomicU64,
    pub state_mutations: AtomicU64,
    pub walls_generated: AtomicU64,
    pub prep_time_ns: AtomicU64,
    pub topo_test_time_ns: AtomicU64,
    pub pbff_time_ns: AtomicU64,
}

static LAZY_SEAL_STATS: LazySealStats = LazySealStats {
    nodes_prepared: AtomicU64::new(0),
    nodes_with_wall_searched: AtomicU64::new(0),
    nodes_with_risky_wall: AtomicU64::new(0),
    heavy_contexts_initialized: AtomicU64::new(0),
    heavy_contexts_used_once: AtomicU64::new(0),
    walls_examined: AtomicU64::new(0),
    topo_safe_walls: AtomicU64::new(0),
    risky_walls: AtomicU64::new(0),
    illegal_risky_walls: AtomicU64::new(0),
    heavy_path_checks: AtomicU64::new(0),
    topo_mask_computes: AtomicU64::new(0),
    pack_wall_array_ops: AtomicU64::new(0),
    state_mutations: AtomicU64::new(0),
    walls_generated: AtomicU64::new(0),
    prep_time_ns: AtomicU64::new(0),
    topo_test_time_ns: AtomicU64::new(0),
    pbff_time_ns: AtomicU64::new(0),
};

pub fn lazy_seal_stats() -> &'static LazySealStats {
    &LAZY_SEAL_STATS
}

pub fn reset_lazy_seal_stats() {
    let s = lazy_seal_stats();
    s.nodes_prepared.store(0, Ordering::Relaxed);
    s.nodes_with_wall_searched.store(0, Ordering::Relaxed);
    s.nodes_with_risky_wall.store(0, Ordering::Relaxed);
    s.heavy_contexts_initialized.store(0, Ordering::Relaxed);
    s.heavy_contexts_used_once.store(0, Ordering::Relaxed);
    s.walls_examined.store(0, Ordering::Relaxed);
    s.topo_safe_walls.store(0, Ordering::Relaxed);
    s.risky_walls.store(0, Ordering::Relaxed);
    s.illegal_risky_walls.store(0, Ordering::Relaxed);
    s.heavy_path_checks.store(0, Ordering::Relaxed);
    s.topo_mask_computes.store(0, Ordering::Relaxed);
    s.pack_wall_array_ops.store(0, Ordering::Relaxed);
    s.state_mutations.store(0, Ordering::Relaxed);
    s.walls_generated.store(0, Ordering::Relaxed);
    s.prep_time_ns.store(0, Ordering::Relaxed);
    s.topo_test_time_ns.store(0, Ordering::Relaxed);
    s.pbff_time_ns.store(0, Ordering::Relaxed);
}

pub fn dump_lazy_seal_stats() -> String {
    let s = lazy_seal_stats();
    let nodes = s.nodes_prepared.load(Ordering::Relaxed);
    format!(
        "lazy_seal nodes={} wall_nodes={} risky_nodes={} heavy_ctx={} heavy_ctx_once={} \
        walls_gen={} walls={} safe={} risky={} illegal_risky={} heavy_checks={} \
        topo_computes={} pack_ops={} mutations={} \
        prep_ms={:.3} topo_ms={:.3} pbff_ms={:.3}",
        nodes,
        s.nodes_with_wall_searched.load(Ordering::Relaxed),
        s.nodes_with_risky_wall.load(Ordering::Relaxed),
        s.heavy_contexts_initialized.load(Ordering::Relaxed),
        s.heavy_contexts_used_once.load(Ordering::Relaxed),
        s.walls_generated.load(Ordering::Relaxed),
        s.walls_examined.load(Ordering::Relaxed),
        s.topo_safe_walls.load(Ordering::Relaxed),
        s.risky_walls.load(Ordering::Relaxed),
        s.illegal_risky_walls.load(Ordering::Relaxed),
        s.heavy_path_checks.load(Ordering::Relaxed),
        s.topo_mask_computes.load(Ordering::Relaxed),
        s.pack_wall_array_ops.load(Ordering::Relaxed),
        s.state_mutations.load(Ordering::Relaxed),
        s.prep_time_ns.load(Ordering::Relaxed) as f64 / 1e6,
        s.topo_test_time_ns.load(Ordering::Relaxed) as f64 / 1e6,
        s.pbff_time_ns.load(Ordering::Relaxed) as f64 / 1e6,
    )
}

#[inline]
fn ace_cell_to_board(cell: usize) -> (u8, u8) {
    ((8 - cell / 9) as u8, (cell % 9) as u8)
}

/// Strategy for the heavy L3 context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LazySealMode {
    /// Build `WallGrids` + PBFF state eagerly in `from_game`.
    EagerHeavy,
    /// Compute only topology masks in `from_game`; initialize heavy context on first risky wall.
    DeferredHeavy,
    /// Legacy reference: mutate `GameState` with `set_wall_bits` + `has_path`.
    Legacy,
}

impl Default for LazySealMode {
    fn default() -> Self {
        Self::DeferredHeavy
    }
}

impl LazySealMode {
    pub fn from_env() -> Self {
        match std::env::var("TITANIUM_LAZY_SEAL_MODE")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("eager") => Self::EagerHeavy,
            Some("legacy") => Self::Legacy,
            _ => Self::DeferredHeavy,
        }
    }
}

/// Heavy L3 trial state, built only when needed.
struct LazyWallTrialCtx {
    grids: WallGrids,
    p0_bit: u128,
    p1_bit: u128,
}

/// Per-`ab()` node: cheap topology masks always; heavy context only on first risky wall.
pub struct LazySealNode {
    pub topo_h: u64,
    pub topo_v: u64,
    pub mode: LazySealMode,
    trial: Option<LazyWallTrialCtx>,
    wall_seen: bool,
    risky_seen: bool,
    heavy_used: u64,
}

impl LazySealNode {
    /// Stage 1: cheap preparation always. Stage 2 eager when mode demands it.
    pub fn from_game(g: &GameState, mode: LazySealMode) -> Self {
        LAZY_SEAL_STATS
            .nodes_prepared
            .fetch_add(1, Ordering::Relaxed);
        let t0 = Instant::now();
        let hw = GameState::ace_wall_bits_to_board(g.hw_bits);
        let vw = GameState::ace_wall_bits_to_board(g.vw_bits);
        LAZY_SEAL_STATS
            .topo_mask_computes
            .fetch_add(1, Ordering::Relaxed);
        let topo_h = wall_needs_flood_h_from_bits(hw, vw);
        let topo_v = wall_needs_flood_v_from_bits(hw, vw);
        LAZY_SEAL_STATS
            .prep_time_ns
            .fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);

        let mut node = Self {
            topo_h,
            topo_v,
            mode,
            trial: None,
            wall_seen: false,
            risky_seen: false,
            heavy_used: 0,
        };
        if mode == LazySealMode::EagerHeavy {
            node.ensure_trial(g);
        }
        node
    }

    /// Stage 2: build heavy context on first risky wall.
    fn ensure_trial(&mut self, g: &GameState) {
        if self.trial.is_some() {
            return;
        }
        LAZY_SEAL_STATS
            .heavy_contexts_initialized
            .fetch_add(1, Ordering::Relaxed);
        let (r0, c0) = ace_cell_to_board(g.pawn[0]);
        let (r1, c1) = ace_cell_to_board(g.pawn[1]);
        let mut board = Board::new();
        board.horizontal_walls = GameState::ace_wall_bits_to_board(g.hw_bits);
        board.vertical_walls = GameState::ace_wall_bits_to_board(g.vw_bits);
        self.trial = Some(LazyWallTrialCtx {
            grids: WallGrids::from_board(&board),
            p0_bit: pawn_bit(r0, c0),
            p1_bit: pawn_bit(r1, c1),
        });
    }

    /// True when the wall can touch enough topology to possibly seal (needs L3).
    #[inline]
    fn needs_flood(&self, wall_type: usize, slot: usize) -> bool {
        let t0 = Instant::now();
        LAZY_SEAL_STATS
            .walls_examined
            .fetch_add(1, Ordering::Relaxed);
        let mask = if wall_type == 0 {
            self.topo_h
        } else {
            self.topo_v
        };
        let board_bit = GameState::ace_slot_to_board_bit(slot);
        let ok = ((mask >> board_bit) & 1) != 0;
        LAZY_SEAL_STATS
            .topo_test_time_ns
            .fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        ok
    }

    /// Exact path legality for a topology-heavy wall — does not mutate `GameState`.
    fn paths_remain_open(&mut self, wall_type: usize, slot: usize) -> bool {
        LAZY_SEAL_STATS
            .heavy_path_checks
            .fetch_add(1, Ordering::Relaxed);
        let t0 = Instant::now();
        // ACE slot row is reflected relative to Board wall row.
        let row = (7 - slot / 8) as u8;
        let col = (slot % 8) as u8;
        let orient = if wall_type == 0 {
            WallOrientation::Horizontal
        } else {
            WallOrientation::Vertical
        };
        let delta = wall_delta(row, col, orient);
        let trial = self.trial.as_mut().expect("heavy context initialized");
        trial.grids.place(delta);
        let ok = pbff_wall_legal(trial.p0_bit, trial.p1_bit, &trial.grids);
        trial.grids.remove(delta);
        LAZY_SEAL_STATS
            .pbff_time_ns
            .fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        ok
    }

    /// Lazy seal gate used from the alpha-beta move loop.
    #[inline]
    pub fn allows_lazy_wall(&mut self, g: &mut GameState, wall_type: usize, slot: usize) -> bool {
        self.wall_seen = true;
        if !self.needs_flood(wall_type, slot) {
            LAZY_SEAL_STATS
                .topo_safe_walls
                .fetch_add(1, Ordering::Relaxed);
            return true;
        }
        LAZY_SEAL_STATS.risky_walls.fetch_add(1, Ordering::Relaxed);
        self.risky_seen = true;
        if self.mode != LazySealMode::Legacy {
            self.ensure_trial(g);
            self.heavy_used += 1;
        }
        let ok = if self.mode == LazySealMode::Legacy {
            legacy_seal_allows(g, wall_type, slot)
        } else {
            self.paths_remain_open(wall_type, slot)
        };
        if !ok {
            LAZY_SEAL_STATS
                .illegal_risky_walls
                .fetch_add(1, Ordering::Relaxed);
        }
        ok
    }
}

impl Drop for LazySealNode {
    fn drop(&mut self) {
        if self.wall_seen {
            LAZY_SEAL_STATS
                .nodes_with_wall_searched
                .fetch_add(1, Ordering::Relaxed);
        }
        if self.risky_seen {
            LAZY_SEAL_STATS
                .nodes_with_risky_wall
                .fetch_add(1, Ordering::Relaxed);
        }
        if self.heavy_used == 1 {
            LAZY_SEAL_STATS
                .heavy_contexts_used_once
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Reference implementation: mutates `GameState` with `set_wall_bits` + twin `has_path`.
pub fn lazy_seal_record_wall_generated() {
    LAZY_SEAL_STATS
        .walls_generated
        .fetch_add(1, Ordering::Relaxed);
}

pub fn legacy_seal_allows(g: &mut GameState, wall_type: usize, slot: usize) -> bool {
    if !g.wall_needs_path_check(wall_type, slot) {
        return true;
    }
    LAZY_SEAL_STATS
        .state_mutations
        .fetch_add(2, Ordering::Relaxed);
    g.set_wall_bits(wall_type, slot, true);
    let ok = g.has_path(0) && g.has_path(1);
    g.set_wall_bits(wall_type, slot, false);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::titanium::algebraic_to_move_id;

    fn pos(moves: &[&str]) -> GameState {
        let mut g = GameState::new();
        for m in moves {
            g.make_move(algebraic_to_move_id(m));
        }
        g
    }

    #[test]
    fn wall_stamp_is_count_not_key() {
        let g1 = pos(&["e2", "e8", "c3h", "f3h"]);
        let g2 = pos(&["e2", "e8", "d3h", "e3h"]);
        assert_eq!(g1.wall_stamp, g2.wall_stamp);
        assert_ne!(g1.hw_bits, g2.hw_bits);
        let n1 = LazySealNode::from_game(&g1, LazySealMode::DeferredHeavy);
        let n2 = LazySealNode::from_game(&g2, LazySealMode::DeferredHeavy);
        assert_ne!(n1.topo_h, n2.topo_h);
    }

    #[test]
    fn hw_bits_tracks_make_unmake() {
        let mut g = GameState::new();
        assert_eq!(g.hw_bits, 0);
        g.make_move(algebraic_to_move_id("d3h"));
        assert_ne!(g.hw_bits, 0);
        let bits_after = g.hw_bits;
        g.make_move(algebraic_to_move_id("e7"));
        g.make_move(algebraic_to_move_id("f3h"));
        assert_ne!(g.hw_bits, bits_after);
        g.unmake_move();
        assert_eq!(g.hw_bits, bits_after);
        g.unmake_move();
        g.unmake_move();
        assert_eq!(g.hw_bits, 0);
        #[cfg(debug_assertions)]
        g.assert_wall_bits_sync();
    }

    #[test]
    fn lazy_seal_matches_legacy_on_wall_fits_positions() {
        let bases = [
            pos(&[]),
            pos(&["e2", "e8", "e3", "e7"]),
            pos(&["e2", "e8", "c3h", "c6h", "e4v"]),
        ];
        for mut g in bases {
            for wall_type in [0usize, 1] {
                for slot in 0..64usize {
                    if !g.wall_fits(wall_type, slot) {
                        continue;
                    }
                    let mut node = LazySealNode::from_game(&g, LazySealMode::DeferredHeavy);
                    let new_ok = node.allows_lazy_wall(&mut g, wall_type, slot);
                    let old_ok = legacy_seal_allows(&mut g, wall_type, slot);
                    assert_eq!(
                        new_ok, old_ok,
                        "seal parity wall_type={wall_type} slot={slot}"
                    );
                }
            }
        }
    }

    #[test]
    fn deferred_does_not_build_trial_until_risky() {
        let mut g = pos(&[]);
        let mut node = LazySealNode::from_game(&g, LazySealMode::DeferredHeavy);
        assert!(node.trial.is_none());
        // A wall slot in the middle of an empty board is topology-safe (no seal possible).
        let slot = 27; // d4h-ish
        let _ = node.allows_lazy_wall(&mut g, 0, slot);
        assert!(
            node.trial.is_none(),
            "empty-board central wall should not need L3"
        );
    }

    #[test]
    fn eager_builds_trial_immediately() {
        let g = pos(&[]);
        let node = LazySealNode::from_game(&g, LazySealMode::EagerHeavy);
        assert!(node.trial.is_some());
    }
}
