//! pathfix/RaceProof — two-phase fixed-topology race solver.
//!
//! Scope: both wall hands are empty, so the blocked-edge topology is fixed.
//!
//! **Phase 1 — theorem sign classifier** (memoized, pruned search):
//!   1. Own-goal distance lead ≥ 2 → forced win/loss sign (bulk fill).
//!   2. Gap ≤ 1 → provisional runner/chaser; chaser too late → runner wins.
//!   3. Else shortest-path continuations only determine sign (not ply depth).
//!
//! **Phase 2 — exact DTM retrograde** (live-only successor cache, ~169 KB scratch):
//!   Build each legal state's successors once, then ply-round fixpoint identical
//!   to the old solver:
//!     win:  `+k = 1 + min losing-child magnitude`
//!     loss: `-k = 1 + max winning-child magnitude`
//!
//! The old full-state successor graph (~200 KB) is gone from production.
//! `solve_race_config_reference` remains under `#[cfg(test)]` only.

use crate::titanium::game::GameState;

/// 81 × 81 × 2 (p0 cell, p1 cell, side to move).
pub const RACE_STATES: usize = 13_122;

/// Legal live pawn placements: p0 ∉ goal row, p1 ∉ goal row, p0 ≠ p1, both turns.
pub const RACE_LIVE_STATES: usize = 10_242;

/// Race-proof score band: above every heuristic eval, below the true-mate band.
/// Table values:
///   +k = side to move wins in k plies,
///   -k = side to move loses in k plies,
///    0 = illegal/unused state.
pub const RACE_MATE: i32 = 32_000;

/// Reusable solver scratch — well below the old ~200 KB full-state successor graph.
/// Live-only successor cache (10,242 × 5) avoids repeated movegen during DTM rounds.
pub struct RaceScratch {
    dist: [[u8; 81]; 2],
    sign: Box<[i8]>,
    graph_slot: Box<[u16]>,
    live: Box<[u16]>,
    nsucc: Box<[u8]>,
    succ: Box<[i16]>,
    buf: [i16; 16],
}

impl RaceScratch {
    pub fn new() -> Self {
        Self {
            dist: [[u8::MAX; 81]; 2],
            sign: vec![0i8; RACE_STATES].into_boxed_slice(),
            graph_slot: vec![0u16; RACE_STATES].into_boxed_slice(),
            live: vec![0u16; RACE_LIVE_STATES].into_boxed_slice(),
            nsucc: vec![0u8; RACE_LIVE_STATES].into_boxed_slice(),
            succ: vec![0i16; RACE_LIVE_STATES * 5].into_boxed_slice(),
            buf: [0; 16],
        }
    }

    /// Total heap scratch excluding the caller-owned table.
    pub const fn scratch_bytes() -> usize {
        std::mem::size_of::<[[u8; 81]; 2]>()
            + std::mem::size_of::<[i16; 16]>()
            + RACE_STATES * (std::mem::size_of::<i8>() + std::mem::size_of::<u16>())
            + RACE_LIVE_STATES * (std::mem::size_of::<u16>() + std::mem::size_of::<u8>())
            + RACE_LIVE_STATES * 5 * std::mem::size_of::<i16>()
    }
}

impl Default for RaceScratch {
    fn default() -> Self {
        Self::new()
    }
}

#[inline(always)]
fn state_id(p0: usize, p1: usize, turn: usize) -> usize {
    (p0 * 81 + p1) * 2 + turn
}

#[inline(always)]
fn decode_state(id: usize) -> (usize, usize, usize) {
    let turn = id % 2;
    let pp = id / 2;
    (pp / 81, pp % 81, turn)
}

#[inline(always)]
fn is_home(side: usize, cell: usize) -> bool {
    if side == 0 {
        cell < 9
    } else {
        cell >= 72
    }
}

#[inline(always)]
fn arrival_ply(side: usize, turn: usize, distance: u8) -> i16 {
    debug_assert!(distance != u8::MAX);
    if distance == 0 {
        0
    } else {
        2 * distance as i16 - i16::from(side == turn)
    }
}

#[inline(always)]
fn cell_manhattan(a: usize, b: usize) -> usize {
    let ar = a / 9;
    let ac = a % 9;
    let br = b / 9;
    let bc = b % 9;
    ar.abs_diff(br) + ac.abs_diff(bc)
}

#[inline(always)]
fn is_jump(from: usize, to: usize) -> bool {
    cell_manhattan(from, to) == 2
}

#[inline(always)]
fn provisional_runner(d0: u8, d1: u8, turn: usize) -> usize {
    if d0 < d1 {
        0
    } else if d1 < d0 {
        1
    } else {
        turn
    }
}

#[inline(always)]
fn chaser_can_reach_runner_goal_no_later(
    g: &GameState,
    runner: usize,
    runner_goal_dist: &[u8; 81],
) -> bool {
    let chaser = runner ^ 1;
    let runner_d = runner_goal_dist[g.pawn[runner]];
    let chaser_d = runner_goal_dist[g.pawn[chaser]];

    if runner_d == u8::MAX {
        return true;
    }
    if chaser_d == u8::MAX {
        return false;
    }

    arrival_ply(chaser, g.turn, chaser_d) <= arrival_ply(runner, g.turn, runner_d)
}

#[inline(always)]
fn sign_from_winner(turn: usize, winner: usize) -> i8 {
    if winner == turn {
        1
    } else {
        -1
    }
}

/// Child sign is from the child side-to-move perspective; flip for parent mover.
#[inline(always)]
fn child_sign_to_parent(child: i8) -> i8 {
    -child
}

#[inline(always)]
fn include_sign_choice(can_win: &mut bool, can_lose: &mut bool, my_sign: i8) {
    debug_assert!(my_sign == 1 || my_sign == -1);
    if my_sign > 0 {
        *can_win = true;
    } else {
        *can_lose = true;
    }
}

fn solve_theorem_sign(
    g: &mut GameState,
    dist0: &[u8; 81],
    dist1: &[u8; 81],
    sign: &mut [i8],
) -> i8 {
    let id = state_id(g.pawn[0], g.pawn[1], g.turn);
    let cached = sign[id];
    if cached != 0 {
        return cached;
    }

    let d0 = dist0[g.pawn[0]];
    let d1 = dist1[g.pawn[1]];
    debug_assert!(d0 != u8::MAX && d1 != u8::MAX);

    let result = if d0.abs_diff(d1) >= 2 {
        let winner = usize::from(d1 < d0);
        sign_from_winner(g.turn, winner)
    } else {
        let runner = provisional_runner(d0, d1, g.turn);
        let chaser = runner ^ 1;
        let runner_goal_dist = if runner == 0 { dist0 } else { dist1 };

        if !chaser_can_reach_runner_goal_no_later(g, runner, runner_goal_dist) {
            sign_from_winner(g.turn, runner)
        } else {
            let mover = g.turn;
            let mover_goal_dist = if mover == 0 { dist0 } else { dist1 };
            let from = g.pawn[mover];
            let old_d = mover_goal_dist[from];
            debug_assert!(old_d != u8::MAX);

            let mut moves = [0i16; 16];
            let nm = g.gen_pawn_moves(&mut moves, 0);
            debug_assert!(nm <= 5);

            let mut can_win = false;
            let mut can_lose = false;
            let mut considered = false;

            for &mv in &moves[..nm] {
                let to = mv as usize;

                if is_home(mover, to) {
                    include_sign_choice(&mut can_win, &mut can_lose, 1);
                    considered = true;
                    continue;
                }

                let new_d = mover_goal_dist[to];
                if new_d == u8::MAX {
                    continue;
                }

                let delta = old_d as i16 - new_d as i16;
                let jump = is_jump(from, to);

                if !jump {
                    debug_assert!(delta <= 1, "adjacent move crossed >1 distance levels");
                    if delta != 1 {
                        continue;
                    }

                    considered = true;
                    let saved_turn = g.turn;
                    g.pawn[mover] = to;
                    g.turn ^= 1;

                    let child = solve_theorem_sign(g, dist0, dist1, sign);
                    let my_sign = child_sign_to_parent(child);

                    g.turn = saved_turn;
                    g.pawn[mover] = from;

                    include_sign_choice(&mut can_win, &mut can_lose, my_sign);
                    continue;
                }

                debug_assert!(delta <= 2, "jump crossed >2 distance levels");

                if mover == chaser && delta <= 1 {
                    considered = true;
                    include_sign_choice(&mut can_win, &mut can_lose, -1);
                    continue;
                }

                if delta != 1 && delta != 2 {
                    continue;
                }

                considered = true;
                let saved_turn = g.turn;
                g.pawn[mover] = to;
                g.turn ^= 1;

                let child = solve_theorem_sign(g, dist0, dist1, sign);
                let my_sign = child_sign_to_parent(child);

                g.turn = saved_turn;
                g.pawn[mover] = from;

                include_sign_choice(&mut can_win, &mut can_lose, my_sign);
            }

            if can_win {
                1
            } else if can_lose || !considered {
                -1
            } else {
                -1
            }
        }
    };

    sign[id] = result;
    result
}

fn fill_theorem_signs(g: &mut GameState, dist0: &[u8; 81], dist1: &[u8; 81], sign: &mut [i8]) {
    sign.fill(0);

    for p0 in 9..81usize {
        for p1 in 0..72usize {
            if p1 == p0 {
                continue;
            }
            let d0 = dist0[p0];
            let d1 = dist1[p1];
            if d0.abs_diff(d1) >= 2 {
                let winner = usize::from(d1 < d0);
                sign[state_id(p0, p1, 0)] = sign_from_winner(0, winner);
                sign[state_id(p0, p1, 1)] = sign_from_winner(1, winner);
            }
        }
    }

    let (saved_p0, saved_p1, saved_turn) = (g.pawn[0], g.pawn[1], g.turn);

    for p0 in 9..81usize {
        for p1 in 0..72usize {
            if p1 == p0 {
                continue;
            }

            g.pawn[0] = p0;
            g.pawn[1] = p1;

            g.turn = 0;
            if sign[state_id(p0, p1, 0)] == 0 {
                let _ = solve_theorem_sign(g, dist0, dist1, sign);
            }

            g.turn = 1;
            if sign[state_id(p0, p1, 1)] == 0 {
                let _ = solve_theorem_sign(g, dist0, dist1, sign);
            }
        }
    }

    g.pawn[0] = saved_p0;
    g.pawn[1] = saved_p1;
    g.turn = saved_turn;
}

fn build_live_graph(
    g: &mut GameState,
    graph_slot: &mut [u16],
    live: &mut [u16],
    nsucc: &mut [u8],
    succ: &mut [i16],
    buf: &mut [i16; 16],
) -> usize {
    graph_slot.fill(0);
    let mut n = 0usize;
    let (saved_p0, saved_p1, saved_turn) = (g.pawn[0], g.pawn[1], g.turn);

    for p0 in 9..81usize {
        g.pawn[0] = p0;
        for p1 in 0..72usize {
            if p1 == p0 {
                continue;
            }
            g.pawn[1] = p1;

            for turn in 0..2usize {
                let id = state_id(p0, p1, turn);
                graph_slot[id] = n as u16;
                live[n] = id as u16;
                g.turn = turn;

                let nm = g.gen_pawn_moves(buf, 0);
                debug_assert!(nm <= 5);
                nsucc[n] = nm as u8;
                let off = n * 5;

                for j in 0..nm {
                    let c = buf[j] as usize;
                    succ[off + j] = if turn == 0 {
                        if c < 9 {
                            -1
                        } else {
                            state_id(c, p1, 1) as i16
                        }
                    } else if c >= 72 {
                        -1
                    } else {
                        state_id(p0, c, 0) as i16
                    };
                }
                n += 1;
            }
        }
    }

    g.pawn[0] = saved_p0;
    g.pawn[1] = saved_p1;
    g.turn = saved_turn;

    debug_assert_eq!(n, RACE_LIVE_STATES);
    n
}

/// Ply-round retrograde DTM using theorem signs and a live-only successor cache.
fn fill_exact_dtm(
    g: &mut GameState,
    sign: &[i8],
    graph_slot: &mut [u16],
    live: &mut [u16],
    nsucc: &mut [u8],
    succ: &mut [i16],
    buf: &mut [i16; 16],
    tbl: &mut [i16],
) {
    tbl.fill(0);

    let n_live = build_live_graph(g, graph_slot, live, nsucc, succ, buf);
    let mut n_unresolved = n_live;
    let mut k = 1i32;

    while n_unresolved > 0 && k < 1024 {
        let mut assigned = 0usize;
        let mut keep = 0usize;

        for i in 0..n_unresolved {
            let id = live[i] as usize;
            debug_assert_ne!(sign[id], 0);

            let gi = graph_slot[id] as usize;
            let ns = nsucc[gi] as usize;
            let off = gi * 5;

            let mut min_loss = i32::MAX;
            let mut all_win = ns > 0;
            let mut max_win = 0i32;

            for j in 0..ns {
                let nid = succ[off + j];
                if nid < 0 {
                    min_loss = min_loss.min(0);
                    all_win = false;
                    continue;
                }

                let v = tbl[nid as usize] as i32;
                if v < 0 {
                    all_win = false;
                    min_loss = min_loss.min(-v);
                } else if v > 0 {
                    max_win = max_win.max(v);
                } else {
                    all_win = false;
                }
            }

            if min_loss != i32::MAX && min_loss + 1 == k {
                tbl[id] = k as i16;
                assigned += 1;
                continue;
            }

            if all_win && max_win + 1 == k {
                tbl[id] = -k as i16;
                assigned += 1;
                continue;
            }

            live[keep] = id as u16;
            keep += 1;
        }

        n_unresolved = keep;
        if assigned == 0 {
            break;
        }
        k += 1;
    }

    debug_assert_eq!(n_unresolved, 0, "DTM pass left {n_unresolved} unresolved states");
}

/// Fill the complete fixed-topology race table: theorem signs + exact DTM.
pub fn solve_race_config(g: &mut GameState, s: &mut RaceScratch, tbl: &mut [i16]) {
    debug_assert_eq!(tbl.len(), RACE_STATES);

    g.compute_dist(0, &mut s.dist[0]);
    g.compute_dist(1, &mut s.dist[1]);

    let dist0 = s.dist[0];
    let dist1 = s.dist[1];

    fill_theorem_signs(g, &dist0, &dist1, &mut s.sign);
    fill_exact_dtm(
        g,
        &s.sign,
        &mut s.graph_slot,
        &mut s.live,
        &mut s.nsucc,
        &mut s.succ,
        &mut s.buf,
        tbl,
    );
}

// ---------------------------------------------------------------------------
// Test-only exhaustive reference oracle.
// ---------------------------------------------------------------------------

#[cfg(test)]
struct ReferenceScratch {
    succ: Box<[i16]>,
    nsucc: Box<[u8]>,
    live: Box<[i32]>,
    buf: [i16; 16],
}

#[cfg(test)]
impl ReferenceScratch {
    fn new() -> Self {
        Self {
            succ: vec![0i16; RACE_STATES * 5].into_boxed_slice(),
            nsucc: vec![0u8; RACE_STATES].into_boxed_slice(),
            live: vec![0i32; RACE_STATES].into_boxed_slice(),
            buf: [0; 16],
        }
    }
}

#[cfg(test)]
fn solve_race_config_reference(g: &mut GameState, s: &mut ReferenceScratch, tbl: &mut [i16]) {
    debug_assert_eq!(tbl.len(), RACE_STATES);
    let (sp0, sp1, sturn) = (g.pawn[0], g.pawn[1], g.turn);
    tbl.fill(0);

    let mut n_live = 0usize;
    for p0 in 9..81usize {
        g.pawn[0] = p0;
        for p1 in 0..72usize {
            if p1 == p0 {
                continue;
            }
            g.pawn[1] = p1;
            let base = state_id(p0, p1, 0);

            g.turn = 0;
            let nm = g.gen_pawn_moves(&mut s.buf, 0);
            debug_assert!(nm <= 5);
            s.nsucc[base] = nm as u8;
            let off = base * 5;
            for j in 0..nm {
                let c = s.buf[j] as usize;
                s.succ[off + j] = if c < 9 { -1 } else { state_id(c, p1, 1) as i16 };
            }
            s.live[n_live] = base as i32;
            n_live += 1;

            g.turn = 1;
            let nm = g.gen_pawn_moves(&mut s.buf, 0);
            debug_assert!(nm <= 5);
            s.nsucc[base + 1] = nm as u8;
            let off = (base + 1) * 5;
            for j in 0..nm {
                let c = s.buf[j] as usize;
                s.succ[off + j] = if c >= 72 {
                    -1
                } else {
                    state_id(p0, c, 0) as i16
                };
            }
            s.live[n_live] = (base + 1) as i32;
            n_live += 1;
        }
    }

    g.pawn[0] = sp0;
    g.pawn[1] = sp1;
    g.turn = sturn;

    let mut k = 1i32;
    while n_live > 0 && k < 1024 {
        let mut assigned = 0usize;
        let mut keep = 0usize;

        for i in 0..n_live {
            let id = s.live[i] as usize;
            let ns = s.nsucc[id] as usize;
            let mut min_loss = 32_767i32;
            let mut all_win = ns > 0;
            let mut max_win = 0i32;
            let off = id * 5;

            for j in 0..ns {
                let nid = s.succ[off + j];
                if nid < 0 {
                    min_loss = 0;
                    all_win = false;
                    continue;
                }

                let v = tbl[nid as usize] as i32;
                if v < 0 {
                    all_win = false;
                    min_loss = min_loss.min(-v);
                } else if v > 0 {
                    max_win = max_win.max(v);
                } else {
                    all_win = false;
                }
            }

            if min_loss + 1 == k {
                tbl[id] = k as i16;
                assigned += 1;
                continue;
            }

            if all_win && max_win + 1 == k {
                tbl[id] = -k as i16;
                assigned += 1;
                continue;
            }

            s.live[keep] = id as i32;
            keep += 1;
        }

        n_live = keep;
        if assigned == 0 {
            break;
        }
        k += 1;
    }
}

#[cfg(test)]
fn gen_successor_ids_for_test(
    g: &mut GameState,
    id: usize,
    buf: &mut [i16; 16],
    succ_out: &mut [i16; 5],
) -> usize {
    let (p0, p1, turn) = decode_state(id);
    g.pawn[0] = p0;
    g.pawn[1] = p1;
    g.turn = turn;

    let nm = g.gen_pawn_moves(buf, 0);
    debug_assert!(nm <= 5);

    for j in 0..nm {
        let c = buf[j] as usize;
        succ_out[j] = if turn == 0 {
            if c < 9 {
                -1
            } else {
                state_id(c, p1, 1) as i16
            }
        } else if c >= 72 {
            -1
        } else {
            state_id(p0, c, 0) as i16
        };
    }
    nm
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solved_empty_board() -> Vec<i16> {
        let mut g = GameState::new();
        let mut s = RaceScratch::new();
        let mut tbl = vec![0i16; RACE_STATES];
        solve_race_config(&mut g, &mut s, &mut tbl);
        tbl
    }

    fn compare_tables(fast: &[i16], reference: &[i16]) -> (usize, usize, usize, Option<(usize, i16, i16)>, Option<(usize, i16, i16)>) {
        let mut live = 0usize;
        let mut sign_mismatches = 0usize;
        let mut exact_mismatches = 0usize;
        let mut first_sign = None;
        let mut first_exact = None;

        for id in 0..RACE_STATES {
            if reference[id] == 0 && fast[id] == 0 {
                continue;
            }
            live += 1;
            if fast[id].signum() != reference[id].signum() {
                sign_mismatches += 1;
                first_sign.get_or_insert((id, fast[id], reference[id]));
            }
            if fast[id] != reference[id] {
                exact_mismatches += 1;
                first_exact.get_or_insert((id, fast[id], reference[id]));
            }
        }

        (live, sign_mismatches, exact_mismatches, first_sign, first_exact)
    }

    fn print_mismatch(label: &str, id: usize, fast: i16, reference: i16) {
        let turn = id % 2;
        let pp = id / 2;
        let p0 = pp / 81;
        let p1 = pp % 81;
        eprintln!("{label}: id={id} p0={p0} p1={p1} turn={turn} fast={fast} ref={reference}");
    }

    #[test]
    fn empty_board_exhaustive_exact_equality() {
        let mut g = GameState::new();

        let mut fast_scratch = RaceScratch::new();
        let mut fast = vec![0i16; RACE_STATES];
        solve_race_config(&mut g, &mut fast_scratch, &mut fast);

        let mut ref_scratch = ReferenceScratch::new();
        let mut reference = vec![0i16; RACE_STATES];
        solve_race_config_reference(&mut g, &mut ref_scratch, &mut reference);

        let (live, sign_m, exact_m, first_sign, first_exact) = compare_tables(&fast, &reference);

        if let Some((id, f, r)) = first_sign {
            print_mismatch("first sign mismatch", id, f, r);
        }
        if let Some((id, f, r)) = first_exact {
            print_mismatch("first exact mismatch", id, f, r);
        }

        eprintln!(
            "empty-board: live={live} sign_mismatches={sign_m} exact_mismatches={exact_m}"
        );

        assert_eq!(sign_m, 0, "sign mismatches on empty board");
        assert_eq!(exact_m, 0, "exact mismatches on empty board");
    }

    #[test]
    fn empty_board_exhaustive_audit_with_benchmark() {
        let mut g = GameState::new();

        const ITERS: u32 = 200;
        let mut sign_us = 0u128;
        let mut dtm_us = 0u128;
        let combined_us;

        for _ in 0..ITERS {
            let mut s = RaceScratch::new();
            let mut tbl = vec![0i16; RACE_STATES];

            g.compute_dist(0, &mut s.dist[0]);
            g.compute_dist(1, &mut s.dist[1]);
            let dist0 = s.dist[0];
            let dist1 = s.dist[1];

            let t0 = std::time::Instant::now();
            fill_theorem_signs(&mut g, &dist0, &dist1, &mut s.sign);
            sign_us += t0.elapsed().as_micros();

            let t1 = std::time::Instant::now();
            fill_exact_dtm(
                &mut g,
                &s.sign,
                &mut s.graph_slot,
                &mut s.live,
                &mut s.nsucc,
                &mut s.succ,
                &mut s.buf,
                &mut tbl,
            );
            dtm_us += t1.elapsed().as_micros();
        }

        let mut fast_scratch = RaceScratch::new();
        let mut fast = vec![0i16; RACE_STATES];
        let t3 = std::time::Instant::now();
        for _ in 0..ITERS {
            solve_race_config(&mut g, &mut fast_scratch, &mut fast);
        }
        combined_us = t3.elapsed().as_micros();

        let t4 = std::time::Instant::now();
        let mut ref_scratch = ReferenceScratch::new();
        let mut reference = vec![0i16; RACE_STATES];
        for _ in 0..ITERS {
            solve_race_config_reference(&mut g, &mut ref_scratch, &mut reference);
        }
        let ref_us = t4.elapsed().as_micros();

        let (live, sign_m, exact_m, _, _) = compare_tables(&fast, &reference);
        let n = u128::from(ITERS);

        eprintln!(
            "benchmark: scratch_bytes={} sign_us={} dtm_us={} combined_us={} ref_us={} speedup={:.2}x live={live} sign_m={sign_m} exact_m={exact_m}",
            RaceScratch::scratch_bytes(),
            sign_us / n,
            dtm_us / n,
            combined_us / n,
            ref_us / n,
            (ref_us as f64 / n as f64) / (combined_us as f64 / n as f64).max(1.0)
        );

        assert_eq!(sign_m, 0);
        assert_eq!(exact_m, 0);
    }

    #[test]
    fn theorem_sign_matches_reference_on_all_sample_configs() {
        use crate::titanium::algebraic_to_move_id;

        let configs: [&[&str]; 3] = [
            &[],
            &["e2", "e8", "e3h", "e6h"],
            &["e2", "e8", "c3h", "f6v", "d7h", "b4v"],
        ];

        for moves in configs {
            let mut g = GameState::new();
            for m in moves {
                g.make_move(algebraic_to_move_id(m));
            }

            let mut fast_scratch = RaceScratch::new();
            let mut fast = vec![0i16; RACE_STATES];
            solve_race_config(&mut g, &mut fast_scratch, &mut fast);

            let mut ref_scratch = ReferenceScratch::new();
            let mut reference = vec![0i16; RACE_STATES];
            solve_race_config_reference(&mut g, &mut ref_scratch, &mut reference);

            let (_, sign_m, exact_m, first_sign, first_exact) = compare_tables(&fast, &reference);

            assert_eq!(
                sign_m, 0,
                "sign mismatch; moves={moves:?}, first={first_sign:?}, exact_m={exact_m}, first_exact={first_exact:?}"
            );
            assert_eq!(
                exact_m, 0,
                "exact mismatch; moves={moves:?}, first={first_exact:?}"
            );
        }
    }

    #[test]
    fn theorem_matches_reference_on_sample_configs() {
        use crate::titanium::algebraic_to_move_id;

        let configs: [&[&str]; 3] = [
            &[],
            &["e2", "e8", "e3h", "e6h"],
            &["e2", "e8", "c3h", "f6v", "d7h", "b4v"],
        ];

        for moves in configs {
            let mut g = GameState::new();
            for m in moves {
                g.make_move(algebraic_to_move_id(m));
            }

            let mut fast_scratch = RaceScratch::new();
            let mut fast = vec![0i16; RACE_STATES];
            solve_race_config(&mut g, &mut fast_scratch, &mut fast);

            let mut ref_scratch = ReferenceScratch::new();
            let mut reference = vec![0i16; RACE_STATES];
            solve_race_config_reference(&mut g, &mut ref_scratch, &mut reference);

            let (_, sign_m, exact_m, first_sign, first_exact) = compare_tables(&fast, &reference);

            assert_eq!(
                sign_m, 0,
                "theorem winner mismatch; moves={moves:?}, first={first_sign:?}, exact_m={exact_m}, first_exact={first_exact:?}"
            );
            assert_eq!(
                exact_m, 0,
                "theorem exact-ply mismatch; moves={moves:?}, first={first_exact:?}"
            );
        }
    }

    #[test]
    fn empty_board_head_on_race_is_movers_loss() {
        let tbl = solved_empty_board();
        let p0 = 76;
        let p1 = 4;
        assert_eq!(tbl[state_id(p0, p1, 0)], -16);
        assert_eq!(tbl[state_id(p0, p1, 1)], -16);
    }

    #[test]
    fn immediate_jump_to_goal_wins_in_one_ply() {
        let tbl = solved_empty_board();
        let p0 = 18;
        let p1 = 9;
        assert_eq!(tbl[state_id(p0, p1, 0)], 1);
    }

    #[test]
    fn race_table_is_bellman_consistent_on_sample_configs() {
        use crate::titanium::algebraic_to_move_id;

        let configs: [&[&str]; 3] = [
            &[],
            &["e2", "e8", "e3h", "e6h"],
            &["e2", "e8", "c3h", "f6v", "d7h", "b4v"],
        ];

        for moves in configs {
            let mut g = GameState::new();
            for m in moves {
                g.make_move(algebraic_to_move_id(m));
            }

            let mut fast_scratch = RaceScratch::new();
            let mut tbl = vec![0i16; RACE_STATES];
            solve_race_config(&mut g, &mut fast_scratch, &mut tbl);

            let mut buf = [0i16; 16];
            let mut succ = [0i16; 5];

            for id in 0..RACE_STATES {
                let v = tbl[id] as i32;
                if v == 0 {
                    continue;
                }

                let ns = gen_successor_ids_for_test(&mut g, id, &mut buf, &mut succ);
                let mut min_loss = i32::MAX;
                let mut all_resolved_win = ns > 0;
                let mut max_win = 0i32;

                for j in 0..ns {
                    let nid = succ[j];
                    if nid < 0 {
                        min_loss = min_loss.min(0);
                        all_resolved_win = false;
                        continue;
                    }

                    let sv = tbl[nid as usize] as i32;
                    if sv < 0 {
                        all_resolved_win = false;
                        min_loss = min_loss.min(-sv);
                    } else if sv > 0 {
                        max_win = max_win.max(sv);
                    } else {
                        all_resolved_win = false;
                    }
                }

                if v > 0 {
                    assert_eq!(v, min_loss + 1, "win value mismatch at state {id}");
                } else {
                    assert!(all_resolved_win, "loss state {id} has a non-win successor");
                    assert_eq!(-v, max_win + 1, "loss value mismatch at state {id}");
                }
            }
        }
    }

    #[test]
    fn ka_game_ply67_stubborn_loser_root_moves() {
        use crate::titanium::algebraic_to_move_id;
        use crate::titanium::move_id_to_algebraic;

        let moves = [
            "e2", "e8", "e3", "e7", "e4", "e6", "e3h", "f6h", "c3h", "d4v", "e5v", "h6h", "a3h",
            "d6h", "f4v", "c5v", "h1h", "b4h", "g5h", "a7h", "f1h", "c7h", "d1h", "e5", "e6", "e4",
            "d6", "f4", "d5", "f5", "d4", "f6", "c4", "g6", "b4", "h6", "a4", "i6", "a5", "i5",
            "b5", "i4", "b6", "h4", "c6", "b6h", "b6", "h3", "a6", "g3", "a7", "f3", "b7", "e3",
            "c7", "d3", "d7", "d2", "e7", "c2", "b1h", "e7h", "d7", "b2", "c7", "a2",
        ];

        let mut g = GameState::new();
        for m in moves {
            g.make_move(algebraic_to_move_id(m));
        }

        let mut s = RaceScratch::new();
        let mut tbl = vec![0i16; RACE_STATES];
        solve_race_config(&mut g, &mut s, &mut tbl);

        let id = state_id(g.pawn[0], g.pawn[1], g.turn);
        let rv = tbl[id] as i32;
        let me = g.turn;
        let mut buf = [0i16; 16];
        let nm = g.gen_pawn_moves(&mut buf, 0);
        let mut best_key = i32::MIN;
        let mut best_alg = String::new();

        for &mv in &buf[..nm] {
            let c = mv as usize;
            let my_v = if is_home(me, c) {
                1
            } else {
                let child_id = if me == 0 {
                    state_id(c, g.pawn[1], 1)
                } else {
                    state_id(g.pawn[0], c, 0)
                };

                let v = tbl[child_id] as i32;
                if v == 0 {
                    continue;
                }

                if v > 0 {
                    -(v + 1)
                } else {
                    1 - v
                }
            };

            let key = if my_v > 0 {
                1_000_000 - my_v
            } else {
                -1_000_000 - my_v
            };

            if key > best_key {
                best_key = key;
                best_alg = move_id_to_algebraic(mv);
            }
        }

        assert!(rv < 0, "white must be in a proven loss");
        assert_eq!(
            best_alg, "b7",
            "b7 and d7 tie on race plies; b7 wins move-order tie-break"
        );
    }

    #[test]
    fn one_step_from_goal_wins_immediately() {
        let tbl = solved_empty_board();
        let p0 = 13;
        let p1 = 40;
        assert_eq!(tbl[state_id(p0, p1, 0)], 1);
    }

    #[test]
    fn random_fixed_topology_exact_equality() {
        use crate::titanium::algebraic_to_move_id;

        let seed: u64 = 0xACE5_2026;
        let mut rng = seed;
        fn next_u64(rng: &mut u64) -> u64 {
            *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            *rng
        }

        // Opening plus wall pool; each trial replays a shuffled prefix with both hands empty.
        let pool: [&str; 24] = [
            "e2", "e8", "e3", "e7", "e4", "e6", "e3h", "f6h", "c3h", "d4v", "e5v", "h6h", "a3h",
            "d6h", "f4v", "c5v", "h1h", "b4h", "g5h", "a7h", "f1h", "c7h", "d1h", "b1h",
        ];

        const N: usize = 100;
        for trial in 0..N {
            let mut order: Vec<usize> = (0..pool.len()).collect();
            for i in 0..order.len() {
                let j = (next_u64(&mut rng) as usize) % order.len();
                order.swap(i, j);
            }

            let n_moves = 8 + (next_u64(&mut rng) as usize) % (pool.len() - 7);
            let mut g = GameState::new();
            for &idx in &order[..n_moves] {
                g.make_move(algebraic_to_move_id(pool[idx]));
            }

            let mut fast_scratch = RaceScratch::new();
            let mut fast = vec![0i16; RACE_STATES];
            solve_race_config(&mut g, &mut fast_scratch, &mut fast);

            let mut ref_scratch = ReferenceScratch::new();
            let mut reference = vec![0i16; RACE_STATES];
            solve_race_config_reference(&mut g, &mut ref_scratch, &mut reference);

            let (_, sign_m, exact_m, first_sign, first_exact) = compare_tables(&fast, &reference);
            if sign_m != 0 || exact_m != 0 {
                eprintln!("random topology failure trial={trial} seed={seed} n_moves={n_moves}");
                if let Some((id, f, r)) = first_sign {
                    print_mismatch("sign", id, f, r);
                }
                if let Some((id, f, r)) = first_exact {
                    print_mismatch("exact", id, f, r);
                }
            }

            assert_eq!(sign_m, 0, "trial {trial} seed {seed}");
            assert_eq!(exact_m, 0, "trial {trial} seed {seed}");
        }
    }
}
