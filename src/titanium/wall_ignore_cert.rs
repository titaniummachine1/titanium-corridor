//! Experimental wall-ignorance forced-loss certificate (Titanium v15 experimental).
//!
//! Feature-gated via `TITANIUM_WALL_IGNORE_LOSS_CERT` (default off).

use crate::core::board::{Board, Player};
use crate::titanium::cert_bridge::{paths_overlap, titanium_game_from_board};
use crate::titanium::dist::fill_ace_dist_from_pawn;
use crate::titanium::game::{GameState, BORDER, DELTA, DIRBIT};
use crate::titanium::race::{RACE_MATE, RACE_WIN_FLOOR};
use crate::titanium::wall_ignore_corridor::{
    detect_zero_delay_corridor, prove_strict_immutable_path_with_stats, shortest_distance,
    CorridorScratch, RunnerGuarantee, StrictPathStats, StrictRunnerGuarantee,
};
use std::sync::atomic::{AtomicU64, Ordering};

pub const FEATURE_ENV: &str = "TITANIUM_WALL_IGNORE_LOSS_CERT";
pub const TRACE_ENV: &str = "TITANIUM_WALL_IGNORE_CERT_TRACE";
pub const IMMUTABLE_FEATURE_ENV: &str = "TITANIUM_IMMUTABLE_PATH_ORACLE";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaceInteraction {
    NonInteracting,
    Deterministic,
    Volatile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertSource {
    WallIgnoranceCorridor,
    StrictImmutablePath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WallIgnoreVerdict {
    pub winner: usize,
    pub winner_terminal_ply: u16,
    pub loser_terminal_ply: u16,
    pub source: CertSource,
    pub interaction: RaceInteraction,
    pub race_minimax_used: bool,
}

#[derive(Default, Debug)]
pub struct WallIgnoreStats {
    pub detector_calls: u64,
    pub corridors_found: u64,
    pub certificates_emitted: u64,
    pub path_edge_checks: u64,
    pub detector_nanos: u64,
}

pub static WALL_IGNORE_STATS: WallIgnoreStatsAtomic = WallIgnoreStatsAtomic::new();
pub static IMMUTABLE_PATH_STATS: ImmutablePathStatsAtomic = ImmutablePathStatsAtomic::new();

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImmutablePathStats {
    pub calls: u64,
    pub pv_calls: u64,
    pub non_pv_calls: u64,
    pub paths_reconstructed: u64,
    pub edges_checked: u64,
    pub unique_wall_candidates: u64,
    pub geometric_rejects: u64,
    pub timing_rejects: u64,
    pub full_legality_checks: u64,
    pub blocking_legal_walls: u64,
    pub guarantees_found: u64,
    pub race_queries: u64,
    pub race_exact_hits: u64,
    pub race_bound_hits: u64,
    pub alpha_beta_cutoffs: u64,
    pub detector_nanos: u64,
    pub oracle_nanos: u64,
}

pub struct ImmutablePathStatsAtomic {
    calls: AtomicU64,
    pv_calls: AtomicU64,
    non_pv_calls: AtomicU64,
    paths_reconstructed: AtomicU64,
    edges_checked: AtomicU64,
    unique_wall_candidates: AtomicU64,
    geometric_rejects: AtomicU64,
    timing_rejects: AtomicU64,
    full_legality_checks: AtomicU64,
    blocking_legal_walls: AtomicU64,
    guarantees_found: AtomicU64,
    race_queries: AtomicU64,
    race_exact_hits: AtomicU64,
    race_bound_hits: AtomicU64,
    alpha_beta_cutoffs: AtomicU64,
    detector_nanos: AtomicU64,
    oracle_nanos: AtomicU64,
}

impl ImmutablePathStatsAtomic {
    const fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
            pv_calls: AtomicU64::new(0),
            non_pv_calls: AtomicU64::new(0),
            paths_reconstructed: AtomicU64::new(0),
            edges_checked: AtomicU64::new(0),
            unique_wall_candidates: AtomicU64::new(0),
            geometric_rejects: AtomicU64::new(0),
            timing_rejects: AtomicU64::new(0),
            full_legality_checks: AtomicU64::new(0),
            blocking_legal_walls: AtomicU64::new(0),
            guarantees_found: AtomicU64::new(0),
            race_queries: AtomicU64::new(0),
            race_exact_hits: AtomicU64::new(0),
            race_bound_hits: AtomicU64::new(0),
            alpha_beta_cutoffs: AtomicU64::new(0),
            detector_nanos: AtomicU64::new(0),
            oracle_nanos: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> ImmutablePathStats {
        ImmutablePathStats {
            calls: self.calls.load(Ordering::Relaxed),
            pv_calls: self.pv_calls.load(Ordering::Relaxed),
            non_pv_calls: self.non_pv_calls.load(Ordering::Relaxed),
            paths_reconstructed: self.paths_reconstructed.load(Ordering::Relaxed),
            edges_checked: self.edges_checked.load(Ordering::Relaxed),
            unique_wall_candidates: self.unique_wall_candidates.load(Ordering::Relaxed),
            geometric_rejects: self.geometric_rejects.load(Ordering::Relaxed),
            timing_rejects: self.timing_rejects.load(Ordering::Relaxed),
            full_legality_checks: self.full_legality_checks.load(Ordering::Relaxed),
            blocking_legal_walls: self.blocking_legal_walls.load(Ordering::Relaxed),
            guarantees_found: self.guarantees_found.load(Ordering::Relaxed),
            race_queries: self.race_queries.load(Ordering::Relaxed),
            race_exact_hits: self.race_exact_hits.load(Ordering::Relaxed),
            race_bound_hits: self.race_bound_hits.load(Ordering::Relaxed),
            alpha_beta_cutoffs: self.alpha_beta_cutoffs.load(Ordering::Relaxed),
            detector_nanos: self.detector_nanos.load(Ordering::Relaxed),
            oracle_nanos: self.oracle_nanos.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        let fields = [
            &self.calls,
            &self.pv_calls,
            &self.non_pv_calls,
            &self.paths_reconstructed,
            &self.edges_checked,
            &self.unique_wall_candidates,
            &self.geometric_rejects,
            &self.timing_rejects,
            &self.full_legality_checks,
            &self.blocking_legal_walls,
            &self.guarantees_found,
            &self.race_queries,
            &self.race_exact_hits,
            &self.race_bound_hits,
            &self.alpha_beta_cutoffs,
            &self.detector_nanos,
            &self.oracle_nanos,
        ];
        for field in fields {
            field.store(0, Ordering::Relaxed);
        }
    }
}

pub struct WallIgnoreStatsAtomic {
    pub detector_calls: AtomicU64,
    pub corridors_found: AtomicU64,
    pub certificates_emitted: AtomicU64,
    pub path_edge_checks: AtomicU64,
    pub detector_nanos: AtomicU64,
}

impl WallIgnoreStatsAtomic {
    pub const fn new() -> Self {
        Self {
            detector_calls: AtomicU64::new(0),
            corridors_found: AtomicU64::new(0),
            certificates_emitted: AtomicU64::new(0),
            path_edge_checks: AtomicU64::new(0),
            detector_nanos: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> WallIgnoreStats {
        WallIgnoreStats {
            detector_calls: self.detector_calls.load(Ordering::Relaxed),
            corridors_found: self.corridors_found.load(Ordering::Relaxed),
            certificates_emitted: self.certificates_emitted.load(Ordering::Relaxed),
            path_edge_checks: self.path_edge_checks.load(Ordering::Relaxed),
            detector_nanos: self.detector_nanos.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.detector_calls.store(0, Ordering::Relaxed);
        self.corridors_found.store(0, Ordering::Relaxed);
        self.certificates_emitted.store(0, Ordering::Relaxed);
        self.path_edge_checks.store(0, Ordering::Relaxed);
        self.detector_nanos.store(0, Ordering::Relaxed);
    }
}

#[inline]
pub fn wall_ignore_loss_cert_enabled() -> bool {
    std::env::var(FEATURE_ENV)
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

#[inline]
pub fn wall_ignore_cert_trace_enabled() -> bool {
    std::env::var(TRACE_ENV)
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

#[inline]
pub fn earliest_terminal_ply(side: usize, side_to_move: usize, distance: u8) -> u16 {
    if distance == 0 {
        return 0;
    }
    let moves_first = side == side_to_move;
    2 * distance as u16 - u16::from(moves_first)
}

pub struct CertScratch {
    pub corridor: CorridorScratch,
}

impl Default for CertScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl CertScratch {
    pub fn new() -> Self {
        Self {
            corridor: CorridorScratch::new(),
        }
    }
}

fn classify_race_interaction(
    g: &GameState,
    _guarantee: &RunnerGuarantee,
    _loser: usize,
) -> RaceInteraction {
    let mut d0 = [0u8; 81];
    let mut d1 = [0u8; 81];
    g.compute_dist(0, &mut d0);
    g.compute_dist(1, &mut d1);
    if paths_overlap(g, &d0, &d1) {
        let adj = crate::titanium::cert_bridge::turn_adjusted_tempo_advantage(g);
        if adj.abs() >= 2 {
            RaceInteraction::Deterministic
        } else {
            RaceInteraction::Volatile
        }
    } else {
        RaceInteraction::NonInteracting
    }
}

fn direct_wall_ignore_verdict(
    g: &GameState,
    winner: usize,
    guarantee: &RunnerGuarantee,
) -> Option<WallIgnoreVerdict> {
    let loser = 1 - winner;
    let winner_ply = earliest_terminal_ply(winner, g.turn, guarantee.max_own_moves_to_goal);
    let loser_dist = shortest_distance(g, loser);
    if loser_dist == 255 {
        return None;
    }
    let loser_ply = earliest_terminal_ply(loser, g.turn, loser_dist);
    if winner_ply >= loser_ply {
        return None;
    }
    Some(WallIgnoreVerdict {
        winner,
        winner_terminal_ply: winner_ply,
        loser_terminal_ply: loser_ply,
        source: CertSource::WallIgnoranceCorridor,
        interaction: RaceInteraction::NonInteracting,
        race_minimax_used: false,
    })
}

fn trace_rejection(
    reason: &str,
    g: &GameState,
    winner: Option<usize>,
    interaction: Option<RaceInteraction>,
) {
    if !wall_ignore_cert_trace_enabled() {
        return;
    }
    eprintln!(
        "wall_ignore_certificate: reject={reason} turn={} p0={} p1={} wl=({}, {}) winner={winner:?} interaction={interaction:?}",
        g.turn, g.pawn[0], g.pawn[1], g.wl[0], g.wl[1]
    );
}

fn trace_verdict(
    g: &GameState,
    guarantee: &RunnerGuarantee,
    verdict: &WallIgnoreVerdict,
    interaction: RaceInteraction,
) {
    if !wall_ignore_cert_trace_enabled() {
        return;
    }
    eprintln!(
        "wall_ignore_certificate: winner={} loser={} path={:?} edges={:?} w_dist={} w_ply={} l_dist={} l_ply={} interaction={interaction:?} race_minimax={} verdict=ForcedWin",
        verdict.winner,
        1 - verdict.winner,
        guarantee.path,
        guarantee.protected_edges,
        guarantee.max_own_moves_to_goal,
        verdict.winner_terminal_ply,
        shortest_distance(g, 1 - verdict.winner),
        verdict.loser_terminal_ply,
        verdict.race_minimax_used,
    );
}

/// Conservative proof that the opponent cannot become adjacent to the
/// certified runner before any remaining runner step. Before first contact,
/// pawn jumps are impossible, so ordinary topology distance is an admissible
/// earliest-contact bound. Future walls only remove movement edges.
fn strict_path_is_temporally_non_interacting(
    g: &GameState,
    guarantee: &StrictRunnerGuarantee,
) -> bool {
    let runner = guarantee.side;
    let opponent = 1 - runner;
    let mut opponent_dist = [u8::MAX; 81];
    fill_ace_dist_from_pawn(g, g.pawn[opponent], &mut opponent_dist);
    let opponent_moves_first = usize::from(g.turn == opponent);

    for edge_index in 0..guarantee.protected_edge_count as usize {
        let runner_cell = guarantee.path[edge_index] as usize;
        let opponent_moves_available = edge_index + opponent_moves_first;
        if opponent_dist[runner_cell] as usize <= opponent_moves_available {
            return false;
        }
        let blocked = g.blocked[runner_cell] | BORDER[runner_cell];
        for direction in 0..4 {
            if blocked & DIRBIT[direction] != 0 {
                continue;
            }
            let adjacent = (runner_cell as i16 + DELTA[direction]) as usize;
            if opponent_dist[adjacent] as usize <= opponent_moves_available {
                return false;
            }
        }
    }
    true
}

/// Project a strict immutable runner path into an exact race result without
/// changing wall inventories or board topology. Exactness is emitted only when
/// the runner path is temporally non-interacting and its terminal ply is
/// strictly earlier than the opponent's earliest possible terminal ply.
pub fn try_strict_immutable_path_race(
    g: &GameState,
    is_pv: bool,
    force_enable: bool,
) -> Option<WallIgnoreVerdict> {
    if !force_enable && !immutable_path_oracle_enabled() {
        return None;
    }
    if g.winner() >= 0 || g.wl[0] + g.wl[1] == 0 {
        return None;
    }

    IMMUTABLE_PATH_STATS.calls.fetch_add(1, Ordering::Relaxed);
    if is_pv {
        IMMUTABLE_PATH_STATS
            .pv_calls
            .fetch_add(1, Ordering::Relaxed);
    } else {
        IMMUTABLE_PATH_STATS
            .non_pv_calls
            .fetch_add(1, Ordering::Relaxed);
    }

    let detector_start = std::time::Instant::now();
    for runner in [g.turn, 1 - g.turn] {
        let mut path_stats = StrictPathStats::default();
        let guarantee = prove_strict_immutable_path_with_stats(g, runner, &mut path_stats);
        record_strict_path_stats(path_stats);
        let Some(guarantee) = guarantee else {
            continue;
        };
        IMMUTABLE_PATH_STATS
            .guarantees_found
            .fetch_add(1, Ordering::Relaxed);

        let oracle_start = std::time::Instant::now();
        IMMUTABLE_PATH_STATS
            .race_queries
            .fetch_add(1, Ordering::Relaxed);
        if !strict_path_is_temporally_non_interacting(g, &guarantee) {
            IMMUTABLE_PATH_STATS
                .oracle_nanos
                .fetch_add(oracle_start.elapsed().as_nanos() as u64, Ordering::Relaxed);
            continue;
        }
        let opponent = 1 - runner;
        let opponent_distance = shortest_distance(g, opponent);
        if opponent_distance == u8::MAX {
            continue;
        }
        let runner_ply = earliest_terminal_ply(runner, g.turn, guarantee.max_own_moves_to_goal);
        let opponent_ply = earliest_terminal_ply(opponent, g.turn, opponent_distance);
        if runner_ply < opponent_ply {
            IMMUTABLE_PATH_STATS
                .race_exact_hits
                .fetch_add(1, Ordering::Relaxed);
            IMMUTABLE_PATH_STATS
                .oracle_nanos
                .fetch_add(oracle_start.elapsed().as_nanos() as u64, Ordering::Relaxed);
            IMMUTABLE_PATH_STATS.detector_nanos.fetch_add(
                detector_start.elapsed().as_nanos() as u64,
                Ordering::Relaxed,
            );
            return Some(WallIgnoreVerdict {
                winner: runner,
                winner_terminal_ply: runner_ply,
                loser_terminal_ply: opponent_ply,
                source: CertSource::StrictImmutablePath,
                interaction: RaceInteraction::NonInteracting,
                race_minimax_used: false,
            });
        }
        IMMUTABLE_PATH_STATS
            .oracle_nanos
            .fetch_add(oracle_start.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    IMMUTABLE_PATH_STATS.detector_nanos.fetch_add(
        detector_start.elapsed().as_nanos() as u64,
        Ordering::Relaxed,
    );
    None
}

#[inline]
pub fn exact_score_from_stm(verdict: &WallIgnoreVerdict, stm: usize) -> i32 {
    debug_assert_eq!(verdict.source, CertSource::StrictImmutablePath);
    let distance = verdict.winner_terminal_ply as i32;
    if verdict.winner == stm {
        RACE_MATE - distance
    } else {
        -RACE_MATE + distance
    }
}

#[inline]
pub fn record_immutable_path_cutoff() {
    IMMUTABLE_PATH_STATS
        .alpha_beta_cutoffs
        .fetch_add(1, Ordering::Relaxed);
}

/// Core detector + race check on a [`GameState`].
pub fn try_wall_ignorance_loss_cert(
    g: &mut GameState,
    scratch: &mut CertScratch,
    force_enable: bool,
) -> Option<WallIgnoreVerdict> {
    if !force_enable && !wall_ignore_loss_cert_enabled() {
        return None;
    }
    if g.winner() >= 0 {
        return None;
    }

    let t0 = std::time::Instant::now();
    WALL_IGNORE_STATS
        .detector_calls
        .fetch_add(1, Ordering::Relaxed);

    for winner in [0usize, 1] {
        let Some(guarantee) = detect_zero_delay_corridor(g, winner, &mut scratch.corridor) else {
            continue;
        };
        WALL_IGNORE_STATS
            .corridors_found
            .fetch_add(1, Ordering::Relaxed);
        WALL_IGNORE_STATS
            .path_edge_checks
            .fetch_add(guarantee.protected_edges.len() as u64, Ordering::Relaxed);

        let loser = 1 - winner;
        let interaction = classify_race_interaction(g, &guarantee, loser);

        let verdict = match interaction {
            RaceInteraction::NonInteracting => direct_wall_ignore_verdict(g, winner, &guarantee),
            RaceInteraction::Deterministic | RaceInteraction::Volatile => {
                trace_rejection("not-non-interacting", g, Some(winner), Some(interaction));
                None
            }
        };

        if let Some(ref v) = verdict {
            if v.winner_terminal_ply >= v.loser_terminal_ply {
                trace_rejection("equal-or-later-winner", g, Some(winner), Some(interaction));
                continue;
            }
            trace_verdict(g, &guarantee, v, interaction);
            WALL_IGNORE_STATS
                .certificates_emitted
                .fetch_add(1, Ordering::Relaxed);
            WALL_IGNORE_STATS
                .detector_nanos
                .fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
            return Some(v.clone());
        }
    }

    WALL_IGNORE_STATS
        .detector_nanos
        .fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
    None
}

/// Board-facing entry (converts to throwaway [`GameState`]).
pub fn try_wall_ignore_cert_board(board: &Board, force_enable: bool) -> Option<WallIgnoreVerdict> {
    let mut g = titanium_game_from_board(board);
    let mut scratch = CertScratch::new();
    try_wall_ignorance_loss_cert(&mut g, &mut scratch, force_enable)
}

/// Proven-outcome bound from the side-to-move perspective.
///
/// `winner_terminal_ply` is a guaranteed arrival estimate, not exact DTM, so
/// this must stay outside both the mate and exact-race score bands.
#[inline]
pub fn cert_score_from_stm(verdict: &WallIgnoreVerdict, stm: usize) -> i32 {
    if verdict.winner == stm {
        RACE_WIN_FLOOR
    } else {
        -RACE_WIN_FLOOR
    }
}

#[inline]
pub fn immutable_path_oracle_enabled() -> bool {
    std::env::var(IMMUTABLE_FEATURE_ENV)
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

fn record_strict_path_stats(stats: StrictPathStats) {
    IMMUTABLE_PATH_STATS
        .paths_reconstructed
        .fetch_add(stats.paths_reconstructed, Ordering::Relaxed);
    IMMUTABLE_PATH_STATS
        .edges_checked
        .fetch_add(stats.edges_checked, Ordering::Relaxed);
    IMMUTABLE_PATH_STATS
        .unique_wall_candidates
        .fetch_add(stats.unique_wall_candidates, Ordering::Relaxed);
    IMMUTABLE_PATH_STATS
        .geometric_rejects
        .fetch_add(stats.geometric_rejects, Ordering::Relaxed);
    IMMUTABLE_PATH_STATS
        .timing_rejects
        .fetch_add(stats.timing_rejects, Ordering::Relaxed);
    IMMUTABLE_PATH_STATS
        .full_legality_checks
        .fetch_add(stats.full_legality_checks, Ordering::Relaxed);
    IMMUTABLE_PATH_STATS
        .blocking_legal_walls
        .fetch_add(stats.blocking_legal_walls, Ordering::Relaxed);
}

#[inline]
pub fn cert_score_from_player(verdict: &WallIgnoreVerdict, player: Player) -> i32 {
    cert_score_from_stm(verdict, player as usize)
}

/// Compare with an existing winner-side certificate; debug builds assert agreement.
pub fn assert_agrees_with_existing(existing_winner: Player, verdict: &WallIgnoreVerdict) {
    debug_assert_eq!(
        existing_winner as usize, verdict.winner,
        "wall-ignore cert disagrees with existing certificate"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with_pawns(p0: usize, p1: usize, turn: usize, wl: (i32, i32)) -> GameState {
        let mut g = GameState::new();
        g.pawn = [p0, p1];
        g.turn = turn;
        g.wl = [wl.0, wl.1];
        g
    }

    /// Column-4 corridor fixture with configurable wall counts.
    fn corridor_game(wl0: i32, wl1: i32) -> GameState {
        let mut g = crate::titanium::wall_ignore_corridor::build_column_four_corridor_fixture();
        g.wl = [wl0, wl1];
        g
    }

    fn strict_near_goal_game() -> GameState {
        let mut g = GameState::new();
        g.pawn = [13, 63];
        g.turn = 0;
        g.wl = [10, 10];
        g
    }

    #[test]
    fn strict_immutable_projection_emits_honest_exact_score() {
        IMMUTABLE_PATH_STATS.reset();
        let g = strict_near_goal_game();
        let original = g.clone();
        let verdict = try_strict_immutable_path_race(&g, true, true).expect("strict race");
        assert_eq!(verdict.source, CertSource::StrictImmutablePath);
        assert_eq!(verdict.winner, 0);
        assert_eq!(verdict.winner_terminal_ply, 1);
        assert!(verdict.winner_terminal_ply < verdict.loser_terminal_ply);
        assert_eq!(exact_score_from_stm(&verdict, 0), RACE_MATE - 1);
        assert_eq!(exact_score_from_stm(&verdict, 1), -RACE_MATE + 1);
        assert_eq!(g.wl, original.wl);
        assert_eq!(g.pawn, original.pawn);
        assert_eq!(g.blocked, original.blocked);
        assert_eq!(g.hash_lo, original.hash_lo);
        assert_eq!(g.hash_hi, original.hash_hi);
        let stats = IMMUTABLE_PATH_STATS.snapshot();
        assert_eq!(stats.calls, 1);
        assert_eq!(stats.pv_calls, 1);
        assert_eq!(stats.race_exact_hits, 1);
    }

    #[test]
    fn strict_projection_declines_pawn_interaction() {
        let mut g = strict_near_goal_game();
        g.pawn[1] = 22;
        assert!(try_strict_immutable_path_race(&g, true, true).is_none());
    }

    #[test]
    fn strict_projection_detects_nontrivial_corridor_fixture() {
        let g = corridor_game(10, 10);
        let verdict = try_strict_immutable_path_race(&g, true, true)
            .expect("separated corridor should be a strict immutable race");
        assert_eq!(verdict.winner, 0);
        assert!(verdict.winner_terminal_ply > 1);
    }

    #[test]
    fn one_tempo_forced_loss_white_wins() {
        let g = corridor_game(10, 10);
        let mut scratch = CertScratch::new();
        let v = try_wall_ignorance_loss_cert(&mut g.clone(), &mut scratch, true).expect("cert");
        assert_eq!(v.winner, 0);
        assert!(v.winner_terminal_ply < v.loser_terminal_ply);
    }

    #[test]
    fn arbitrary_loser_wall_count_invariant() {
        for wl1 in [0, 1, 3, 7, 10] {
            let mut g = corridor_game(5, wl1);
            let mut scratch = CertScratch::new();
            let v =
                try_wall_ignorance_loss_cert(&mut g, &mut scratch, true).expect("cert wl1={wl1}");
            assert_eq!(v.winner, 0);
            assert!(v.winner_terminal_ply < v.loser_terminal_ply);
        }
    }

    #[test]
    fn arbitrary_winner_wall_count_invariant() {
        for wl0 in [0, 1, 5, 10] {
            let mut g = corridor_game(wl0, 10);
            let mut scratch = CertScratch::new();
            let v =
                try_wall_ignorance_loss_cert(&mut g, &mut scratch, true).expect("cert wl0={wl0}");
            assert_eq!(v.winner, 0);
        }
    }

    #[test]
    fn winner_zero_walls_still_certifies() {
        let mut g = corridor_game(0, 10);
        let mut scratch = CertScratch::new();
        assert!(try_wall_ignorance_loss_cert(&mut g, &mut scratch, true).is_some());
    }

    #[test]
    fn equal_arrival_no_direct_certificate() {
        // Equal distance 4 both sides, white to move → both ply 7.
        let mut g = game_with_pawns(5 * 9 + 1, 5 * 9 + 7, 0, (10, 10));
        let mut scratch = CertScratch::new();
        assert!(
            try_wall_ignorance_loss_cert(&mut g, &mut scratch, true).is_none(),
            "equal terminal ply must not direct-certify"
        );
    }

    #[test]
    fn candidate_winner_later_no_certificate() {
        // White far, black close — no forced white win.
        let mut g = game_with_pawns(8 * 9 + 4, 2 * 9 + 4, 0, (10, 10));
        let mut scratch = CertScratch::new();
        assert!(try_wall_ignorance_loss_cert(&mut g, &mut scratch, true).is_none());
    }

    #[test]
    fn side_to_move_tempo_differs() {
        let g_stm0 = corridor_game(10, 10);
        let mut g_stm1 = corridor_game(10, 10);
        g_stm1.turn = 1;
        let mut s0 = CertScratch::new();
        let mut s1 = CertScratch::new();
        let v0 = try_wall_ignorance_loss_cert(&mut g_stm0.clone(), &mut s0, true).expect("w stm");
        let v1 = try_wall_ignorance_loss_cert(&mut g_stm1, &mut s1, true);
        assert_eq!(v0.winner, 0);
        if let Some(v1) = v1 {
            assert_ne!(
                v0.winner, v1.winner,
                "side-to-move flip must change which side the certificate favors"
            );
        }
    }

    #[test]
    fn shared_path_no_raw_distance_certificate() {
        let g = game_with_pawns(6 * 9 + 4, 5 * 9 + 4, 0, (0, 0));
        let mut scratch = CertScratch::new();
        assert!(try_wall_ignorance_loss_cert(&mut g.clone(), &mut scratch, true).is_none());
    }

    #[test]
    fn feature_disabled_returns_none_without_env() {
        let g = corridor_game(10, 10);
        let mut scratch = CertScratch::new();
        let prev = std::env::var(FEATURE_ENV).ok();
        std::env::remove_var(FEATURE_ENV);
        assert!(try_wall_ignorance_loss_cert(&mut g.clone(), &mut scratch, false).is_none());
        if let Some(v) = prev {
            std::env::set_var(FEATURE_ENV, v);
        }
    }

    #[test]
    fn earliest_terminal_ply_examples() {
        assert_eq!(earliest_terminal_ply(0, 0, 1), 1);
        assert_eq!(earliest_terminal_ply(0, 1, 1), 2);
        assert_eq!(earliest_terminal_ply(0, 0, 4), 7);
        assert_eq!(earliest_terminal_ply(0, 1, 4), 8);
    }

    #[test]
    fn cert_score_is_a_bound_and_does_not_fake_dtm_ordering() {
        let fast = WallIgnoreVerdict {
            winner: 0,
            winner_terminal_ply: 2,
            loser_terminal_ply: 5,
            source: CertSource::WallIgnoranceCorridor,
            interaction: RaceInteraction::NonInteracting,
            race_minimax_used: false,
        };
        let slow = WallIgnoreVerdict {
            winner: 0,
            winner_terminal_ply: 5,
            loser_terminal_ply: 8,
            source: CertSource::WallIgnoranceCorridor,
            interaction: RaceInteraction::NonInteracting,
            race_minimax_used: false,
        };
        assert_eq!(cert_score_from_stm(&fast, 0), RACE_WIN_FLOOR);
        assert_eq!(cert_score_from_stm(&slow, 0), RACE_WIN_FLOOR);
        assert_eq!(cert_score_from_stm(&fast, 1), -RACE_WIN_FLOOR);
        assert!(fast.winner_terminal_ply < slow.winner_terminal_ply);
    }
}
