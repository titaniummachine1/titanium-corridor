//! Cache-first **projected** hands-empty race probes for low-wall endgames.
//!
//! A projected probe asks: “if both wall stocks were already zero on this fixed
//! topology and pawn placement, what does the exact race oracle say?” That is
//! sound for the counterfactual position but **not** automatically sound for the
//! real position while stocks remain.

use std::time::Instant;

/// Observation trigger: probe suffixes when estimated remaining plies ≤ this.
pub const PROJECTION_OBSERVE_PLIES: i32 = 20;
/// Stronger integration (ordering / wall q-search) threshold.
pub const PROJECTION_QSEARCH_PLIES: i32 = 10;
/// Minimum pawn-only suffix length before probing.
pub const PROJECTION_SUFFIX_MIN: usize = 4;
/// Maximum suffix states probed per completed ID iteration.
pub const PROJECTION_SUFFIX_MAX_PROBES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectedProbeMode {
    CacheOnly,
    CheapGates,
    AllowBudgetedBuild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectedRaceSource {
    ExistingRaceTableCache,
    ExistingLowWallCache,
    CheapDistanceGate,
    HeavyTableBuild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectedRaceResult {
    pub winner: usize,
    pub dtm: Option<i16>,
    pub score: i32,
    pub source: ProjectedRaceSource,
}

#[derive(Clone, Debug)]
pub struct ProjectedRaceBudget {
    pub max_builds_per_move: u32,
    pub max_builds_per_iteration: u32,
    pub max_probes_per_iteration: u32,
    pub max_time_fraction: f64,
    pub hard_time_cap_ms: u64,
    pub builds_this_move: u32,
    pub builds_this_iteration: u32,
    pub probes_this_iteration: u32,
    pub projection_ns: u64,
    think_start: Option<Instant>,
    think_budget_ms: u64,
}

impl Default for ProjectedRaceBudget {
    fn default() -> Self {
        Self {
            max_builds_per_move: 1,
            max_builds_per_iteration: 1,
            max_probes_per_iteration: PROJECTION_SUFFIX_MAX_PROBES as u32,
            max_time_fraction: 0.02,
            hard_time_cap_ms: 10,
            builds_this_move: 0,
            builds_this_iteration: 0,
            probes_this_iteration: 0,
            projection_ns: 0,
            think_start: None,
            think_budget_ms: 0,
        }
    }
}

impl ProjectedRaceBudget {
    pub fn begin_think(&mut self, budget_ms: u64) {
        *self = Self::default();
        self.think_start = Some(Instant::now());
        self.think_budget_ms = budget_ms;
    }

    pub fn begin_iteration(&mut self) {
        self.builds_this_iteration = 0;
        self.probes_this_iteration = 0;
    }

    pub fn can_probe(&self) -> bool {
        self.probes_this_iteration < self.max_probes_per_iteration
    }

    pub fn can_build(&self) -> bool {
        if self.builds_this_move >= self.max_builds_per_move {
            return false;
        }
        if self.builds_this_iteration >= self.max_builds_per_iteration {
            return false;
        }
        let projection_ms = self.projection_ns / 1_000_000;
        let frac_cap = (self.think_budget_ms as f64 * self.max_time_fraction) as u64;
        let cap = self.hard_time_cap_ms.min(frac_cap.max(1));
        if projection_ms >= cap {
            return false;
        }
        true
    }

    pub fn note_probe(&mut self, dt_ns: u64) {
        self.probes_this_iteration += 1;
        self.projection_ns += dt_ns;
    }

    pub fn note_build(&mut self) {
        self.builds_this_move += 1;
        self.builds_this_iteration += 1;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RaceProjectionConfig {
    pub enabled: bool,
    pub cache_only: bool,
    pub allow_build: bool,
    pub ordering: bool,
    pub wall_qsearch: bool,
    pub null_cutoff: bool,
    pub observe_only: bool,
}

impl RaceProjectionConfig {
    pub fn baseline_v17() -> Self {
        Self::default()
    }

    pub fn observe() -> Self {
        Self {
            enabled: true,
            observe_only: true,
            ..Self::default()
        }
    }

    pub fn candidate() -> Self {
        Self {
            enabled: true,
            ordering: true,
            wall_qsearch: true,
            allow_build: true,
            ..Self::default()
        }
    }

    pub fn candidate_null() -> Self {
        Self {
            enabled: true,
            ordering: true,
            wall_qsearch: true,
            allow_build: true,
            null_cutoff: true,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RaceProjectionStats {
    pub trigger_checks: u64,
    pub triggered: u64,
    pub short_pv: u64,
    pub suffix_nodes: u64,
    pub real_exact_hits: u64,
    pub projected_cache_hits: u64,
    pub gate_hits: u64,
    pub cache_misses: u64,
    pub builds: u64,
    pub build_denied_budget: u64,
    pub winner_flips: u64,
    pub stable: u64,
    pub projection_ns: u64,
    pub ordering_nodes: u64,
    pub root_move_changed: u64,
    pub pawn_move_boosts: u64,
    pub wall_move_boosts: u64,
    pub wall_move_demotions: u64,
    pub qsearch_entries: u64,
    pub null_attempts: u64,
    pub null_cutoffs: u64,
    pub exact_real_descendants: u64,
    pub budget_aborts: u64,
}

impl RaceProjectionStats {
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"trigger_checks":{},"triggered":{},"short_pv":{},"suffix_nodes":{},"real_exact_hits":{},"projected_cache_hits":{},"gate_hits":{},"cache_misses":{},"builds":{},"build_denied_budget":{},"winner_flips":{},"stable":{},"projection_ns":{},"ordering_nodes":{},"root_move_changed":{},"pawn_move_boosts":{},"wall_move_boosts":{},"wall_move_demotions":{},"qsearch_entries":{},"null_attempts":{},"null_cutoffs":{},"exact_real_descendants":{},"budget_aborts":{}}}"#,
            self.trigger_checks,
            self.triggered,
            self.short_pv,
            self.suffix_nodes,
            self.real_exact_hits,
            self.projected_cache_hits,
            self.gate_hits,
            self.cache_misses,
            self.builds,
            self.build_denied_budget,
            self.winner_flips,
            self.stable,
            self.projection_ns,
            self.ordering_nodes,
            self.root_move_changed,
            self.pawn_move_boosts,
            self.wall_move_boosts,
            self.wall_move_demotions,
            self.qsearch_entries,
            self.null_attempts,
            self.null_cutoffs,
            self.exact_real_descendants,
            self.budget_aborts,
        )
    }
}

/// Conservative remaining-plies estimate from BFS goal distances at the root.
#[inline]
pub fn estimated_remaining_plies(white_dist: u8, black_dist: u8, walls_left: i32) -> i32 {
    let d0 = if white_dist == 255 {
        16
    } else {
        white_dist as i32
    };
    let d1 = if black_dist == 255 {
        16
    } else {
        black_dist as i32
    };
    d0 + d1 + walls_left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_cache_only_never_builds_by_default() {
        let b = ProjectedRaceBudget::default();
        assert_eq!(b.max_builds_per_move, 1);
        assert!(!b.can_build() || b.builds_this_move == 0);
    }

    #[test]
    fn estimated_plies_increases_with_walls() {
        assert!(estimated_remaining_plies(4, 5, 3) > estimated_remaining_plies(4, 5, 0));
    }
}
