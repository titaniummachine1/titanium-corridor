//! Root LMR plan snapshot — mirrors alphabeta root move list + planned reductions.

use crate::cat::build::{build_impact_heatmap, build_impact_heatmap_for_stm};
use crate::cat::constants::DIST_PENALTY;
use crate::cat::prune::{
    get_shortest_path, is_lmr_heat_hot, is_tactical_move, move_impact_heat, order_moves,
    path_distance,
};
use crate::cat::CorridorAttention;
use crate::core::board::{Board, Move};
use crate::movegen::{generate_legal_moves_slice, MAX_LEGAL_MOVES};
use crate::path::BfsScratch;
use crate::search::cat_index_lmr::CAT_ATTENTION_TAIL_CUTOFF;
use crate::search::cat_index_lmr::{cat_index_lmr_reduction, lmr_tuning_to_aggression_g};
use crate::search::lmr_profile::{compute_stage_t, LmrProfile};
use crate::util::perft::format_move;

const LMR_MIN_DEPTH: u32 = 2;

#[derive(Debug, Clone)]
pub struct RootLmrPlan {
    pub mv: String,
    pub is_pawn: bool,
    pub order: usize,
    pub cat_cm: i32,
    pub tactical: bool,
    pub hot: bool,
    pub cold: bool,
    pub protected: bool,
    pub pruned: bool,
    pub baseline_reduction_fp: f64,
    pub baseline_reduction: u32,
    pub baseline_child_depth_full: u32,
    pub baseline_child_depth_used: u32,
    pub requested_reduction_fp: f64,
    pub reduction: u32,
    pub child_depth_full: u32,
    pub child_depth_used: u32,
    pub reduction_clamped: bool,
    pub in_full_window: bool,
    pub attention_ratio: f64,
    pub dead_tail: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LmrPlanSummary {
    pub protected_moves_changed: u32,
    pub moves_more_reduction: u32,
    pub avg_baseline_reduction_fp: f64,
    pub avg_adjusted_reduction_fp: f64,
    pub max_baseline_reduction: u32,
    pub max_adjusted_reduction: u32,
    pub hot_count: u32,
    pub cold_count: u32,
}

fn lmr_move_protected(
    move_index: usize,
    child_depth_full: u32,
    depth: u32,
    tactical: bool,
) -> bool {
    move_index == 0 || child_depth_full <= 1 || depth < LMR_MIN_DEPTH || tactical
}

fn scale_lmr_reduction(
    cat_cm: i32,
    max_move_impact: u32,
    move_rank: usize,
    move_count: usize,
    child_depth_full: u32,
    _depth: u32,
    lmr_tuning_percent: i32,
    first_reducible_rank: usize,
    protected: bool,
) -> (bool, f64, u32, u32, f64, u32, u32, bool) {
    if protected {
        return (
            true,
            0.0,
            0,
            child_depth_full,
            0.0,
            0,
            child_depth_full,
            false,
        );
    }

    let baseline_g = 1.0;
    let requested_g = lmr_tuning_to_aggression_g(lmr_tuning_percent);

    let baseline_reduction = cat_index_lmr_reduction(
        child_depth_full,
        move_rank,
        move_count,
        cat_cm,
        max_move_impact,
        baseline_g,
        false,
        first_reducible_rank,
    );
    let effective_reduction = cat_index_lmr_reduction(
        child_depth_full,
        move_rank,
        move_count,
        cat_cm,
        max_move_impact,
        requested_g,
        false,
        first_reducible_rank,
    );

    let baseline_used = child_depth_full.saturating_sub(baseline_reduction);
    let child_used = child_depth_full.saturating_sub(effective_reduction);
    let max_safe = child_depth_full;

    (
        false,
        baseline_reduction as f64,
        baseline_reduction,
        baseline_used,
        effective_reduction as f64,
        effective_reduction,
        child_used,
        effective_reduction > max_safe,
    )
}

fn root_cat_heat_stats(moves: &[Move], n: usize, cat: &CorridorAttention) -> (u16, u16) {
    let mut heats = Vec::with_capacity(n);
    for mv in &moves[..n] {
        heats.push(move_impact_heat(*mv, cat).max(0) as u16);
    }
    if heats.is_empty() {
        return (0, 0);
    }
    heats.sort_by(|a, b| b.cmp(a));
    let max = heats[0];
    let p75_idx = (heats.len() * 3 / 4).min(heats.len() - 1);
    (max, heats[p75_idx])
}

fn summarize_plans(plans: &[RootLmrPlan]) -> LmrPlanSummary {
    let mut summary = LmrPlanSummary::default();
    let mut baseline_sum = 0.0;
    let mut adjusted_sum = 0.0;
    let mut n_eligible = 0u32;
    for p in plans {
        if p.hot {
            summary.hot_count += 1;
        }
        if p.cold {
            summary.cold_count += 1;
        }
        if p.protected {
            if p.reduction > 0 {
                summary.protected_moves_changed += 1;
            }
            continue;
        }
        n_eligible += 1;
        baseline_sum += p.baseline_reduction_fp;
        adjusted_sum += p.requested_reduction_fp;
        summary.max_baseline_reduction = summary.max_baseline_reduction.max(p.baseline_reduction);
        summary.max_adjusted_reduction = summary.max_adjusted_reduction.max(p.reduction);
        if p.reduction > p.baseline_reduction {
            summary.moves_more_reduction += 1;
        }
    }
    if n_eligible > 0 {
        let n = f64::from(n_eligible);
        summary.avg_baseline_reduction_fp = baseline_sum / n;
        summary.avg_adjusted_reduction_fp = adjusted_sum / n;
    }
    summary
}

/// Planned root LMR for `id_depth` at `pierce_fraction` elapsed (0 = pierce peak).
pub fn plan_root_lmr(
    board: &mut Board,
    bfs: &mut BfsScratch,
    id_depth: u32,
    time_ms: u64,
    pierce_fraction: f32,
    depth_kept_percent: i32,
) -> (LmrProfile, Vec<RootLmrPlan>) {
    let root_side = board.side();
    let opp_side = root_side.opposite();
    let our_dist = bfs
        .shortest_distance(board, root_side)
        .unwrap_or(DIST_PENALTY);
    let opp_dist = bfs
        .shortest_distance(board, opp_side)
        .unwrap_or(DIST_PENALTY);
    let endgame_race = our_dist.min(opp_dist) <= 4;

    let mut opp_path = [0u8; 81];
    let opp_path_len = get_shortest_path(board, opp_side, bfs, &mut opp_path);
    let opp_dist_path = path_distance(opp_side, &opp_path, opp_path_len);

    // Symmetric view: both players' corridors visible, no STM rear-zeroing.
    // Walls that block the opponent's path register even when they're "behind" us.
    let cat = build_impact_heatmap(board);

    let mut buf = [Move::Pawn { row: 1, col: 4 }; MAX_LEGAL_MOVES];
    let n = generate_legal_moves_slice(board, &mut buf, bfs);
    if n == 0 {
        return (LmrProfile::from_stage(0.5, endgame_race, false), Vec::new());
    }

    let (cat_max_seed, cat_p75) = root_cat_heat_stats(&buf, n, &cat);
    let stage_t = compute_stage_t(board, our_dist, opp_dist, cat_max_seed, cat_p75);

    let mut profile = LmrProfile::from_stage(stage_t, endgame_race, false);
    profile.apply_time_budget(time_ms);
    profile.apply_pierce_schedule(pierce_fraction, time_ms);

    let mut scores = [0i32; MAX_LEGAL_MOVES];
    order_moves(
        board,
        &mut buf,
        n,
        None,
        None,
        &mut scores,
        our_dist,
        opp_dist_path,
        &opp_path,
        opp_path_len,
        bfs,
        &cat,
        &crate::cat::prune::OrderExtras::default(),
        |_| 0,
    );

    let mut cat_values = Vec::with_capacity(n);
    let mut max_move_impact = 0u32;
    for i in 0..n {
        let mv = buf[i];
        let cm = move_impact_heat(mv, &cat);
        cat_values.push(cm);
        max_move_impact = max_move_impact.max(cm.max(0) as u32);
    }

    let depth = id_depth.max(1);
    let child_depth_full = depth.saturating_sub(1);
    let first_reducible_rank = profile.lmr_after_move.saturating_add(1).max(2);

    let mut plans = Vec::with_capacity(n);

    for i in 0..n {
        let mv = buf[i];
        let cat_cm = cat_values[i];
        let move_rank = i + 1;
        let heat_ratio_hot = is_lmr_heat_hot(
            cat_cm,
            max_move_impact as u16,
            profile.cold_cm,
            profile.hot_ratio_pct,
        );
        let cold = cat_cm < i32::from(profile.cold_cm);
        let is_tactical = if i == 0 || depth < LMR_MIN_DEPTH {
            true
        } else if matches!(mv, Move::Wall { .. })
            && !crate::cat::prune::wall_intersects_path(mv, &opp_path, opp_path_len)
        {
            false
        } else {
            is_tactical_move(board, mv, our_dist, opp_dist_path, bfs)
        };
        let protected = lmr_move_protected(i, child_depth_full, depth, is_tactical);

        let (
            protected_flag,
            baseline_fp,
            baseline_reduction,
            baseline_used,
            requested_fp,
            effective_reduction,
            child_used,
            clamped,
        ) = scale_lmr_reduction(
            cat_cm,
            max_move_impact,
            move_rank,
            n,
            child_depth_full,
            depth,
            depth_kept_percent,
            first_reducible_rank,
            protected,
        );

        let in_full_window = child_used >= child_depth_full.saturating_sub(1);
        let attention = if max_move_impact > 0 {
            cat_cm.max(0) as f64 / max_move_impact as f64
        } else {
            0.0
        };
        let dead_tail =
            !protected_flag && max_move_impact > 0 && attention <= CAT_ATTENTION_TAIL_CUTOFF;

        plans.push(RootLmrPlan {
            mv: format_move(mv),
            is_pawn: matches!(mv, Move::Pawn { .. }),
            order: i,
            cat_cm,
            tactical: is_tactical,
            hot: heat_ratio_hot || attention >= 0.72,
            cold,
            protected: protected_flag,
            pruned: false,
            baseline_reduction_fp: baseline_fp,
            baseline_reduction,
            baseline_child_depth_full: child_depth_full,
            baseline_child_depth_used: baseline_used,
            requested_reduction_fp: requested_fp,
            reduction: effective_reduction,
            child_depth_full,
            child_depth_used: child_used,
            reduction_clamped: clamped,
            in_full_window,
            attention_ratio: attention,
            dead_tail,
        });
    }

    (profile, plans)
}

pub fn lmr_profile_fields(profile: &LmrProfile, id_depth: u32) -> String {
    format!(
        "{{\"stageT\":{:.3},\"aggression\":{:.2},\"pierceT\":{:.3},\"moveWindow\":{},\"lmrAfter\":{},\"hotPct\":{},\"coldCm\":{},\"idDepth\":{}}}",
        profile.stage_t,
        profile.aggression,
        profile.pierce_t,
        profile.move_window,
        profile.lmr_after_move,
        profile.hot_ratio_pct,
        profile.cold_cm,
        id_depth,
    )
}

pub fn format_lmr_plans_json(plans: &[RootLmrPlan]) -> String {
    let summary = summarize_plans(plans);
    let mut out = String::new();
    for (i, p) in plans.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"move\":\"{}\",\"kind\":\"{}\",\"order\":{},\"catCm\":{},\"tactical\":{},\"hot\":{},\"cold\":{},\"protected\":{},\"pruned\":{},\
\"baselineReductionFp\":{:.4},\"baselineReduction\":{},\"baselineChildDepthFull\":{},\"baselineChildDepthUsed\":{},\
\"requestedReductionFp\":{:.4},\"reduction\":{},\"childDepthFull\":{},\"childDepthUsed\":{},\"reductionClamped\":{},\"inFullWindow\":{},\"attentionRatio\":{:.4},\"deadTail\":{}}}",
            p.mv,
            if p.is_pawn { "pawn" } else { "wall" },
            p.order,
            p.cat_cm,
            p.tactical,
            p.hot,
            p.cold,
            p.protected,
            p.pruned,
            p.baseline_reduction_fp,
            p.baseline_reduction,
            p.baseline_child_depth_full,
            p.baseline_child_depth_used,
            p.requested_reduction_fp,
            p.reduction,
            p.child_depth_full,
            p.child_depth_used,
            p.reduction_clamped,
            p.in_full_window,
            p.attention_ratio,
            p.dead_tail,
        ));
    }
    format!(
        "\"summary\":{{\"protectedMovesChanged\":{},\"movesMoreReduction\":{},\"avgBaselineReductionFp\":{:.4},\"avgAdjustedReductionFp\":{:.4},\"maxBaselineReduction\":{},\"maxAdjustedReduction\":{},\"hotCount\":{},\"coldCount\":{}}},\"moves\":[{}]",
        summary.protected_moves_changed,
        summary.moves_more_reduction,
        summary.avg_baseline_reduction_fp,
        summary.avg_adjusted_reduction_fp,
        summary.max_baseline_reduction,
        summary.max_adjusted_reduction,
        summary.hot_count,
        summary.cold_count,
        out,
    )
}

/// Pre-search LMR plan — static profile at pierce peak.
pub fn lmr_snapshot_json(
    board: &mut Board,
    time_ms: u64,
    id_depth: u32,
    depth_kept_percent: i32,
) -> String {
    let mut bfs = BfsScratch::new();
    let depth = id_depth.clamp(4, 32);
    let (profile, plans) = plan_root_lmr(board, &mut bfs, depth, time_ms, 0.0, depth_kept_percent);
    format!(
        "{{\"source\":\"shallow\",\"idDepth\":{},\"timeMs\":{},\"lmrAggressionPercent\":{},\"lmrTuningPercent\":{},\"lmrProfile\":{},{}}}",
        depth,
        time_ms,
        depth_kept_percent.clamp(-500, 150),
        depth_kept_percent.clamp(-500, 150),
        lmr_profile_fields(&profile, depth),
        format_lmr_plans_json(&plans),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    use crate::cat::constants::CAT_COLD_CM;
    use crate::cat::prune::is_cat_hot_corridor;
    use crate::search::lmr_profile::TIME_REFERENCE_MS;

    #[test]
    fn cat_hot_cold_threshold_semantics() {
        assert!(is_cat_hot_corridor(160));
        assert!(!is_cat_hot_corridor(159));
        assert!(59 < i32::from(CAT_COLD_CM));
        assert!(!(60 < i32::from(CAT_COLD_CM)));
    }

    #[test]
    fn shallow_snapshot_has_legal_moves() {
        let mut board = Board::new();
        let mut bfs = BfsScratch::new();
        let (_, plans) = plan_root_lmr(&mut board, &mut bfs, 8, TIME_REFERENCE_MS, 0.0, 100);
        assert!(plans.len() >= 4);
        assert!(plans[0].tactical);
    }

    #[test]
    fn zero_tuning_max_reduces_without_protecting_eligible_moves() {
        let mut board = Board::new();
        board.apply_algebraic("e2");
        board.apply_algebraic("e8");
        board.apply_algebraic("e3");
        let mut bfs = BfsScratch::new();
        let (_, plans) = plan_root_lmr(&mut board, &mut bfs, 8, TIME_REFERENCE_MS, 0.0, 0);
        let late_wall = plans
            .iter()
            .filter(|p| !p.is_pawn && p.order > 4)
            .find(|p| !p.protected && p.baseline_reduction_fp > 0.0);
        if let Some(p) = late_wall {
            assert!(p.reduction > 0);
            assert!(
                !p.protected,
                "0% tuning must not mark eligible moves protected"
            );
        }
    }

    #[test]
    fn lmr_vision_cat_cm_matches_impact_heatmap() {
        let mut board = Board::new();
        for m in ["e2", "e8", "e3", "e7", "e4", "e6"] {
            board.apply_algebraic(m);
        }
        let fixture = board.clone();
        let mut bfs = BfsScratch::new();
        let cat = build_impact_heatmap(&fixture);
        let mut work = board;
        let mut legal_buf = [Move::Pawn { row: 0, col: 0 }; MAX_LEGAL_MOVES];
        let mut legal_board = fixture.clone();
        let legal_n = generate_legal_moves_slice(&mut legal_board, &mut legal_buf, &mut bfs);
        let (_, plans) = plan_root_lmr(&mut work, &mut bfs, 8, TIME_REFERENCE_MS, 0.0, 100);
        assert_eq!(
            plans.len(),
            plans.iter().map(|p| &p.mv).collect::<HashSet<_>>().len(),
            "duplicate moves"
        );
        let by_mv: HashMap<String, i32> = plans.iter().map(|p| (p.mv.clone(), p.cat_cm)).collect();
        assert_eq!(by_mv.len(), legal_n, "one plan per legal move");
        for mv in &legal_buf[..legal_n] {
            let key = format_move(*mv);
            let reported = *by_mv.get(&key).unwrap_or_else(|| panic!("missing {key}"));
            let expected = move_impact_heat(*mv, &cat);
            assert_eq!(reported, expected, "LMR vision mismatch for {key}");
        }
    }

    #[test]
    fn tuning_150_disables_adjusted_reduction() {
        let mut board = Board::new();
        let mut bfs = BfsScratch::new();
        let (_, plans) = plan_root_lmr(&mut board, &mut bfs, 8, TIME_REFERENCE_MS, 0.0, 150);
        assert!(plans.len() >= 100);
        assert!(
            plans
                .iter()
                .all(|p| p.reduction == 0 && p.child_depth_used == p.child_depth_full),
            "200% tuning → full depth for all"
        );
    }

    #[test]
    fn tuning_slider_is_monotone_around_default() {
        let mut board = Board::new();
        let mut bfs = BfsScratch::new();
        let id_depth = 8;
        let child_depth_full = id_depth - 1;
        let (_, max_cut) = plan_root_lmr(&mut board, &mut bfs, id_depth, TIME_REFERENCE_MS, 0.0, 0);
        let (_, default_cut) =
            plan_root_lmr(&mut board, &mut bfs, id_depth, TIME_REFERENCE_MS, 0.0, 100);
        let (_, full_depth) =
            plan_root_lmr(&mut board, &mut bfs, id_depth, TIME_REFERENCE_MS, 0.0, 150);
        for ((a, b), c) in max_cut
            .iter()
            .zip(default_cut.iter())
            .zip(full_depth.iter())
        {
            assert_eq!(a.mv, b.mv);
            assert_eq!(a.mv, c.mv);
            assert!(
                a.reduction >= b.reduction && b.reduction >= c.reduction,
                "{} reductions should decrease as tuning rises: {} >= {} >= {}",
                a.mv,
                a.reduction,
                b.reduction,
                c.reduction
            );
            assert_eq!(
                c.child_depth_used, child_depth_full,
                "{} should be full-depth at 150%",
                c.mv
            );
        }
    }

    #[test]
    fn tail_cutoff_at_ten_percent_of_peak() {
        let hmax = 621u32;
        let dead = scale_lmr_reduction(40, hmax, 8, 20, 10, 11, 0, 2, false);
        let fringe = scale_lmr_reduction(77, hmax, 8, 20, 10, 11, 0, 2, false);
        let side = scale_lmr_reduction(400, hmax, 8, 20, 10, 11, 0, 2, false);

        assert_eq!(dead.6, 0, "40/621 ≈ 6.4% → dead tail → child d0 (leaf)");
        assert!(fringe.6 > dead.6, "77/621 ≈ 12.4% should survive hard tail");
        assert!(side.6 > fringe.6, "400/621 should keep more depth than 77");
    }

    #[test]
    fn zero_tuning_is_cat_ratio_shaped_not_flat() {
        let hmax = 1000u32;
        let cold = scale_lmr_reduction(10, hmax, 8, 20, 10, 11, 0, 2, false);
        let mid = scale_lmr_reduction(500, hmax, 8, 20, 10, 11, 0, 2, false);
        let hot = scale_lmr_reduction(1000, hmax, 1, 20, 10, 11, 0, 2, false);

        assert_eq!(cold.6, 0, "1% CAT should leaf-eval at child d0");
        assert!(
            mid.6 > cold.6 && mid.6 < hot.6,
            "50% CAT should keep an intermediate child depth"
        );
        assert_eq!(
            hot.6, 10,
            "100% CAT on first move should keep full child depth"
        );
    }

    #[test]
    fn negative_tuning_overrides_cat_shape_toward_absolute_max_cut() {
        let hmax = 1000u32;
        let hot_cat_shaped = scale_lmr_reduction(1000, hmax, 1, 20, 10, 11, 0, 2, false);
        let cold_absolute = scale_lmr_reduction(10, hmax, 8, 20, 10, 11, -500, 2, false);

        assert_eq!(
            hot_cat_shaped.6, 10,
            "0% still honors a 100% CAT first move"
        );
        assert_eq!(
            cold_absolute.6, 0,
            "-500% forces leaf eval on dead-tail move"
        );
    }

    #[test]
    fn protected_moves_changed_stays_zero() {
        let mut board = Board::new();
        let mut bfs = BfsScratch::new();
        for pct in [-500, 0, 50, 100, 150] {
            let (_, plans) = plan_root_lmr(&mut board, &mut bfs, 8, TIME_REFERENCE_MS, 0.0, pct);
            let summary = summarize_plans(&plans);
            assert_eq!(
                summary.protected_moves_changed, 0,
                "protected must not change at {pct}%"
            );
        }
    }
}
