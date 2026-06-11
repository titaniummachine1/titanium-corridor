//! ACE v7 search — 1:1 port of the JS `Search` object.
//!
//! Iterative-deepening αβ with aspiration windows, typed TT, killers/history/
//! countermoves, null move, graduated LMR, frontier LMP, reverse futility,
//! lazy wall legality, repetition detection, wall-stamp dist caching,
//! easy-move early stop, HalfPW net eval. Mirrors the JS node-for-node.

use crate::util::clock::{Duration, Instant};

use crate::ace::game::{AceGame, ZOBRIST};
use crate::ace::net::{net, Net, NET_BKT, NET_H, NET_MIRC, NET_MIRS};
use crate::cat::build::build_corridor_attention;
use crate::cat::prune::{gap_play_zone_mask, get_shortest_path, wall_should_search};
use crate::cat::CorridorAttention;
use crate::core::board::{Board, Move as BoardMove, Player, Undo, WallOrientation};
use crate::movegen::{generate_legal_moves_slice, MAX_LEGAL_MOVES};
use crate::path::BfsScratch;
use crate::util::grid::{square_index, unpack_square};

pub const MATE: i32 = 100_000;
pub const MAX_PLY: usize = 64;
const INF: i32 = 2 * MATE;
/// αβ plies from the reply position to catch one-wall race traps shallow ID misses.
const TRAP_PROOF_PLIES: i32 = 10;

const TT_BITS: usize = 20;
const TT_SIZE: usize = 1 << TT_BITS;
const TT_MASK: u32 = (TT_SIZE - 1) as u32;

/// Time-abort marker — propagates like the JS `throw "time"`.
pub struct TimeUp;

/// ACE move encoding → Titanium board move (row flip between coordinate systems).
pub fn ace_move_to_board(m: i16) -> BoardMove {
    if m < 100 {
        BoardMove::Pawn {
            row: 8 - (m / 9) as u8,
            col: (m % 9) as u8,
        }
    } else {
        let (base, orientation) = if m < 200 {
            (100, WallOrientation::Horizontal)
        } else {
            (200, WallOrientation::Vertical)
        };
        let slot = m - base;
        BoardMove::Wall {
            row: 7 - (slot / 8) as u8,
            col: (slot % 8) as u8,
            orientation,
        }
    }
}

/// Titanium `Board` kept in sync with the ACE game — fast movegen + optional CAT.
pub struct TiBridge {
    pub board: Board,
    pub bfs: BfsScratch,
    undo_stack: Vec<Undo>,
}

impl TiBridge {
    fn from_game(g: &AceGame) -> Box<Self> {
        let mut board = Board::new();
        for i in 0..g.hist_len {
            let _ = board.make_move(ace_move_to_board(g.hist_m[i]));
        }
        Box::new(Self {
            board,
            bfs: BfsScratch::new(),
            undo_stack: Vec::with_capacity(256),
        })
    }

    fn push(&mut self, m: i16) {
        let undo = self.board.make_move(ace_move_to_board(m));
        self.undo_stack.push(undo);
    }

    fn pop(&mut self) {
        if let Some(undo) = self.undo_stack.pop() {
            self.board.unmake_move(undo);
        }
    }

    /// Full legal moves via Titanium `movegen` → ACE encoding.
    fn gen_legal_ace(&mut self, out: &mut [i16; 160]) -> usize {
        let mut ti_buf = [BoardMove::Pawn { row: 0, col: 0 }; MAX_LEGAL_MOVES];
        let n = generate_legal_moves_slice(&mut self.board, &mut ti_buf, &mut self.bfs);
        for i in 0..n {
            out[i] = board_move_to_ace(ti_buf[i]);
        }
        n
    }
}

/// Titanium board move → ACE numeric encoding.
pub fn board_move_to_ace(mv: BoardMove) -> i16 {
    match mv {
        BoardMove::Pawn { row, col } => ((8 - row as i16) * 9 + col as i16) as i16,
        BoardMove::Wall {
            row,
            col,
            orientation,
        } => {
            let slot = (7 - row as i16) * 8 + col as i16;
            match orientation {
                WallOrientation::Horizontal => 100 + slot,
                WallOrientation::Vertical => 200 + slot,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AceDepthLogEntry {
    pub depth: i32,
    pub score: i32,
    pub nodes: u64,
    pub elapsed_ms: u64,
    pub marginal_nodes: u64,
    pub pv: String,
}

pub struct ThinkResult {
    pub mv: i16,
    pub score: i32,
    pub depth: i32,
    pub nodes: u64,
    pub ms: u64,
    pub white_dist: u8,
    pub black_dist: u8,
    pub depth_log: Vec<AceDepthLogEntry>,
}

fn emit_ace_progress(
    engine_label: &str,
    depth_log: &[AceDepthLogEntry],
    search_depth: i32,
    nodes: u64,
    root_score: i32,
    white_dist: u8,
    black_dist: u8,
    elapsed_ms: u64,
) {
    let mut depth_json = String::new();
    for (i, e) in depth_log.iter().enumerate() {
        if i > 0 {
            depth_json.push(',');
        }
        let pv = e.pv.replace('\\', "\\\\").replace('"', "\\\"");
        depth_json.push_str(&format!(
            "{{\"depth\":{},\"score\":{},\"nodes\":{},\"elapsedMs\":{},\"marginalNodes\":{},\"pv\":\"{}\"}}",
            e.depth, e.score, e.nodes, e.elapsed_ms, e.marginal_nodes, pv
        ));
    }
    eprintln!(
        "info json {{\"engine\":\"{}\",\"stoppedBy\":\"{}\",\"searchDepth\":{},\"nodes\":{},\"rootScore\":{},\"whiteDist\":{},\"blackDist\":{},\"elapsedMs\":{},\"depthLog\":[{}]}}",
        engine_label,
        engine_label,
        search_depth,
        nodes,
        root_score,
        white_dist,
        black_dist,
        elapsed_ms,
        depth_json
    );
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

/// xorshift64 — deterministic, allocation-free playout randomness.
#[inline(always)]
fn next_rand(rng: &mut u64) -> u64 {
    let mut x = *rng;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *rng = x;
    x
}

/// CAT heat threshold for tactically hot rollout moves (matches MCTS rollouts).
const ROLLOUT_HOT_CM: u16 = 160;

fn cat_heat_board_move(mv: BoardMove, cat: &CorridorAttention) -> u16 {
    match mv {
        BoardMove::Pawn { row, col } => cat.square_heat(row, col),
        BoardMove::Wall {
            row,
            col,
            orientation,
        } => cat.wall_edge_heat(row, col, orientation),
    }
}

fn pick_hot_rollout_board(
    legal: &[BoardMove; MAX_LEGAL_MOVES],
    n: usize,
    cat: &CorridorAttention,
    rng: &mut u64,
) -> BoardMove {
    const MAX_HOT: usize = 64;
    let mut hot = [BoardMove::Pawn { row: 0, col: 0 }; MAX_HOT];
    let mut hot_n = 0usize;
    for i in 0..n {
        if cat_heat_board_move(legal[i], cat) >= ROLLOUT_HOT_CM && hot_n < MAX_HOT {
            hot[hot_n] = legal[i];
            hot_n += 1;
        }
    }
    if hot_n > 0 && next_rand(rng) % 3 < 2 {
        return hot[(next_rand(rng) as usize) % hot_n];
    }
    legal[(next_rand(rng) as usize) % n]
}

/// Stream rollout verification stats as an `info json` line on stderr.
#[allow(clippy::too_many_arguments)]
fn emit_rollout_stats(
    engine_label: &str,
    mv: i16,
    attempts: u32,
    spine_plies: u32,
    leaf_score: i32,
    search_score: i32,
    verdict: &str,
) {
    eprintln!(
        "info json {{\"engine\":\"{}\",\"rolloutMove\":\"{}\",\"rolloutAttempts\":{},\"rolloutSpinePlies\":{},\"rolloutLeafScore\":{},\"rolloutSearchScore\":{},\"rolloutVerdict\":\"{}\"}}",
        engine_label,
        super::ace_to_algebraic(mv),
        attempts,
        spine_plies,
        leaf_score,
        search_score,
        verdict
    );
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

pub struct AceSearch {
    pub g: AceGame,
    tt_key_hi: Vec<u32>,
    tt_key_lo: Vec<u32>,
    tt_meta: Vec<i32>, // move | flag<<10 | depth<<12, 0 = empty
    tt_score: Vec<i32>,
    history_tbl: [i32; 512],
    cm: [i16; 512], // countermove table
    killers: [[i16; 2]; MAX_PLY],
    path_lo: [u32; MAX_PLY],
    path_hi: [u32; MAX_PLY],
    d0: [[u8; 81]; MAX_PLY],
    d1: [[u8; 81]; MAX_PLY],
    dist0_idx: usize, // active ply slot in d0 (JS: this.dist0 array ref)
    dist1_idx: usize,
    cached_stamp: i32,
    // HalfPW accumulator cache
    np_acc0: [f64; NET_H],
    np_acc1: [f64; NET_H],
    np_hw: [u8; 64],
    np_vw: [u8; 64],
    np_b0: i32,
    np_b1v: i32,
    net: &'static Net,
    /// Mirrored Titanium board (movegen and/or CAT).
    bridge: Option<Box<TiBridge>>,
    /// Use Titanium `generate_legal_moves_slice` instead of ACE `wall_legal`.
    ti_movegen: bool,
    /// CAT-filter walls at inner nodes (requires `bridge`).
    cat_walls: bool,
    /// Rollout verification of the root best move (minimax-MCTS hybrid):
    /// greedy playouts to terminal positions confirm or contest the αβ choice
    /// and steer deepening/early-stop. αβ keeps final move authority.
    pseudo_mcts: bool,
    pub nodes: u64,
    deadline: Instant,
    root_best: i16,
    root_score: i32,
}

impl AceSearch {
    pub fn new(g: AceGame) -> Box<Self> {
        Box::new(Self {
            g,
            tt_key_hi: vec![0; TT_SIZE],
            tt_key_lo: vec![0; TT_SIZE],
            tt_meta: vec![0; TT_SIZE],
            tt_score: vec![0; TT_SIZE],
            history_tbl: [0; 512],
            cm: [0; 512],
            killers: [[0; 2]; MAX_PLY],
            path_lo: [0; MAX_PLY],
            path_hi: [0; MAX_PLY],
            d0: [[0; 81]; MAX_PLY],
            d1: [[0; 81]; MAX_PLY],
            dist0_idx: 0,
            dist1_idx: 0,
            cached_stamp: -1,
            np_acc0: [0.0; NET_H],
            np_acc1: [0.0; NET_H],
            np_hw: [0; 64],
            np_vw: [0; 64],
            np_b0: -1,
            np_b1v: -1,
            net: net(),
            bridge: None,
            ti_movegen: false,
            cat_walls: false,
            pseudo_mcts: false,
            nodes: 0,
            deadline: Instant::now(),
            root_best: 0,
            root_score: 0,
        })
    }

    /// Enable rollout verification of the root best move (pseudo-MCTS):
    /// between ID iterations the main move is played out to terminal
    /// positions; contradictions force deeper αβ proof instead of early stop.
    pub fn enable_pseudo_mcts(&mut self) {
        self.pseudo_mcts = true;
    }

    /// Titanium movegen on a mirrored board — same legal set, much faster than `wall_legal`.
    pub fn with_ti_movegen(g: AceGame) -> Box<Self> {
        let mut search = Self::new(g);
        search.bridge = Some(TiBridge::from_game(&search.g));
        search.ti_movegen = true;
        search
    }

    /// CAT hybrid: walls at inner nodes must pass `wall_should_search`.
    pub fn with_cat(g: AceGame) -> Box<Self> {
        let mut search = Self::new(g);
        search.bridge = Some(TiBridge::from_game(&search.g));
        search.cat_walls = true;
        search
    }

    /// Fast Titanium movegen + CAT wall filter.
    pub fn with_ti_movegen_and_cat(g: AceGame) -> Box<Self> {
        let mut search = Self::with_ti_movegen(g);
        search.cat_walls = true;
        search
    }

    /// Advance the live game one ply, keeping TT/killers/history warm.
    /// Long-lived session path — the next `think` reuses prior analysis.
    pub fn apply_move(&mut self, m: i16) {
        self.g.make_move(m);
        self.position_changed();
    }

    /// Replace the position outright (undo, new game) without clearing the
    /// TT — entries are hash-keyed, stale ones simply never match.
    pub fn set_position(&mut self, g: AceGame) {
        self.g = g;
        self.position_changed();
    }

    fn position_changed(&mut self) {
        if self.bridge.is_some() {
            self.bridge = Some(TiBridge::from_game(&self.g));
        }
        self.cached_stamp = -1;
        self.np_b0 = -1; // force full accumulator rebuild (v10: no stamp gate)
        self.np_b1v = -1;
    }

    #[inline(always)]
    fn check_time(&self) -> Result<(), TimeUp> {
        if (self.nodes & 1023) == 0 && Instant::now() > self.deadline {
            return Err(TimeUp);
        }
        Ok(())
    }

    fn refresh_dist(&mut self, ply: usize) {
        let stamp = self.g.wall_stamp;
        if self.cached_stamp == stamp {
            return; // refs already valid for these walls
        }
        if self.cached_stamp == stamp - 1 && self.g.hist_len > 0 {
            // exactly one wall added since the cached config: slots hold its dists.
            // recompute a player's field only if the wall cuts a shortest-path edge
            // (|dist diff| === 1); equal-dist edges lie on no shortest path.
            let m = self.g.hist_m[self.g.hist_len - 1];
            if m >= 100 {
                let slot = (m % 100) as usize;
                let a = (slot >> 3) * 9 + (slot & 7);
                let (b2, c2, e2) = if m < 200 {
                    (a + 9, a + 1, a + 10) // hw: two vertical edges
                } else {
                    (a + 1, a + 9, a + 10) // vw: two horizontal edges
                };
                let d0 = &self.d0[self.dist0_idx];
                if d0[a] != d0[b2] || d0[c2] != d0[e2] {
                    self.dist0_idx = ply; // redirect first: never write an ancestor's array
                    self.g.compute_dist(0, &mut self.d0[ply]);
                }
                let d1 = &self.d1[self.dist1_idx];
                if d1[a] != d1[b2] || d1[c2] != d1[e2] {
                    self.dist1_idx = ply;
                    self.g.compute_dist(1, &mut self.d1[ply]);
                }
                self.cached_stamp = stamp;
                return;
            }
        }
        self.dist0_idx = ply; // own arrays: ancestors stay intact
        self.dist1_idx = ply;
        self.g.compute_dist(0, &mut self.d0[ply]);
        self.g.compute_dist(1, &mut self.d1[ply]);
        self.cached_stamp = stamp;
    }

    fn evaluate(&mut self) -> i32 {
        let me = self.g.turn;
        let opp = 1 - me;
        let d_me_u = if me == 0 {
            self.d0[self.dist0_idx][self.g.pawn[0]]
        } else {
            self.d1[self.dist1_idx][self.g.pawn[1]]
        };
        let d_opp_u = if opp == 0 {
            self.d0[self.dist0_idx][self.g.pawn[0]]
        } else {
            self.d1[self.dist1_idx][self.g.pawn[1]]
        };
        let w_me_i = self.g.wl[me];
        let w_opp_i = self.g.wl[opp];
        let d_me_i = d_me_u as i32;
        let d_opp_i = d_opp_u as i32;
        if w_me_i == 0 && w_opp_i == 0 {
            // pure race
            if d_me_i <= d_opp_i {
                return 3000 + (d_opp_i - d_me_i) * 50 - d_me_i;
            }
            return -3000 - (d_me_i - d_opp_i) * 50 + d_opp_i;
        }

        let d_me = d_me_i as f64;
        let d_opp = d_opp_i as f64;
        let w_me = w_me_i as f64;
        let w_opp = w_opp_i as f64;
        let nw = self.net;
        let ws = &nw.ws;

        let pd = d_opp - d_me;
        let wd = w_me - w_opp;
        let mut out = ws[0]
            + ws[1] * pd
            + ws[2] * wd
            + ws[3] * d_me
            + ws[4] * d_opp
            + ws[9] * pd * (w_me + w_opp) / 20.0
            + ws[10] * wd * (d_me + d_opp) / 16.0;
        if w_opp_i == 0 {
            out += ws[6];
            if d_me <= d_opp {
                out += ws[5];
            }
        } else if w_me_i == 0 {
            out += ws[8];
            if d_opp <= d_me - 1.0 {
                out += ws[7];
            }
        }
        if d_opp <= 4.0 {
            out += ws[11] * if w_me < 3.0 { w_me } else { 3.0 };
        }
        if d_me <= 4.0 {
            out += ws[12] * if w_opp < 3.0 { w_opp } else { 3.0 };
        }

        let b0 = NET_BKT[self.g.pawn[0]] as i32;
        let b1 = NET_BKT[NET_MIRC[self.g.pawn[1]]] as i32;
        if b0 != self.np_b0 || b1 != self.np_b1v {
            // bucket cross: rebuild BOTH perspectives (ACE v10 audit blocker 5:
            // rebuilding only the crossed side dropped pending wall diffs for
            // the other accumulator)
            self.np_acc0.fill(0.0);
            self.np_acc1.fill(0.0);
            for s in 0..64 {
                if self.g.hw[s] != 0 {
                    let o = (b0 as usize * 128 + s) * NET_H;
                    for j in 0..NET_H {
                        self.np_acc0[j] += nw.w1c[o + j];
                    }
                    let o = (b1 as usize * 128 + NET_MIRS[s]) * NET_H;
                    for j in 0..NET_H {
                        self.np_acc1[j] += nw.w1c[o + j];
                    }
                }
                if self.g.vw[s] != 0 {
                    let o = (b0 as usize * 128 + 64 + s) * NET_H;
                    for j in 0..NET_H {
                        self.np_acc0[j] += nw.w1c[o + j];
                    }
                    let o = (b1 as usize * 128 + 64 + NET_MIRS[s]) * NET_H;
                    for j in 0..NET_H {
                        self.np_acc1[j] += nw.w1c[o + j];
                    }
                }
                self.np_hw[s] = self.g.hw[s];
                self.np_vw[s] = self.g.vw[s];
            }
            self.np_b0 = b0;
            self.np_b1v = b1;
        } else {
            // NO stamp gate (ACE v10 audit blocker 4: wall_stamp is a count,
            // aliases across sibling wall configs): always diff the wall snapshot
            for s in 0..64 {
                if self.g.hw[s] != self.np_hw[s] {
                    let sg = if self.g.hw[s] != 0 { 1.0 } else { -1.0 };
                    let o0 = (b0 as usize * 128 + s) * NET_H;
                    let o1 = (b1 as usize * 128 + NET_MIRS[s]) * NET_H;
                    for j in 0..NET_H {
                        self.np_acc0[j] += sg * nw.w1c[o0 + j];
                        self.np_acc1[j] += sg * nw.w1c[o1 + j];
                    }
                    self.np_hw[s] = self.g.hw[s];
                }
                if self.g.vw[s] != self.np_vw[s] {
                    let sg = if self.g.vw[s] != 0 { 1.0 } else { -1.0 };
                    let o0 = (b0 as usize * 128 + 64 + s) * NET_H;
                    let o1 = (b1 as usize * 128 + 64 + NET_MIRS[s]) * NET_H;
                    for j in 0..NET_H {
                        self.np_acc0[j] += sg * nw.w1c[o0 + j];
                        self.np_acc1[j] += sg * nw.w1c[o1 + j];
                    }
                    self.np_vw[s] = self.g.vw[s];
                }
            }
        }

        let mut hid = [0.0f64; NET_H];
        if me == 0 {
            for j in 0..NET_H {
                hid[j] = nw.b1[j] + self.np_acc0[j];
            }
            let o0 = self.g.pawn[0] * NET_H;
            for j in 0..NET_H {
                hid[j] += nw.po[o0 + j];
            }
            let o1 = self.g.pawn[1] * NET_H;
            for j in 0..NET_H {
                hid[j] += nw.px[o1 + j];
            }
        } else {
            for j in 0..NET_H {
                hid[j] = nw.b1[j] + self.np_acc1[j];
            }
            let o0 = NET_MIRC[self.g.pawn[1]] * NET_H;
            for j in 0..NET_H {
                hid[j] += nw.po[o0 + j];
            }
            let o1 = NET_MIRC[self.g.pawn[0]] * NET_H;
            for j in 0..NET_H {
                hid[j] += nw.px[o1 + j];
            }
        }
        for j in 0..NET_H {
            let a2 = hid[j].clamp(0.0, 1.0);
            out += nw.w2[j] * a2 * 200.0;
        }
        out as i32
    }

    fn gen_moves(&mut self, ply: usize, depth: i32, tt_move: i16, out: &mut [i16; 160]) -> usize {
        let check_legal = ply == 0;
        if self.ti_movegen && check_legal {
            return self
                .bridge
                .as_mut()
                .expect("ti movegen needs bridge")
                .gen_legal_ace(out);
        }
        let mut n = self.g.gen_pawn_moves(out, 0);
        if self.g.wl[self.g.turn] <= 0 {
            return n;
        }
        if self.cat_walls && !check_legal {
            return self.gen_walls_cat_filtered(depth, tt_move, out, n);
        }
        for slot in 0..64 {
            if check_legal {
                if self.g.wall_legal(0, slot) {
                    out[n] = 100 + slot as i16;
                    n += 1;
                }
                if self.g.wall_legal(1, slot) {
                    out[n] = 200 + slot as i16;
                    n += 1;
                }
            } else {
                // lazy: geometry only; path-seal checked when the move is searched
                if self.g.wall_fits(0, slot) {
                    out[n] = 100 + slot as i16;
                    n += 1;
                }
                if self.g.wall_fits(1, slot) {
                    out[n] = 200 + slot as i16;
                    n += 1;
                }
            }
        }
        n
    }

    /// Hybrid wall generation: lazy geometry + CAT relevance filter.
    ///
    /// CAT (multi-route corridor heat) only above the leaf layer — depth-1 nodes
    /// dominate the tree and only need witness-path tactics, not breadth
    /// (mirrors `search::alphabeta`). The TT move always survives the filter.
    fn gen_walls_cat_filtered(
        &mut self,
        depth: i32,
        tt_move: i16,
        out: &mut [i16; 160],
        mut n: usize,
    ) -> usize {
        let me = self.g.turn;
        let our_dist = if me == 0 {
            self.d0[self.dist0_idx][self.g.pawn[0]]
        } else {
            self.d1[self.dist1_idx][self.g.pawn[1]]
        };
        let opp_dist = if me == 0 {
            self.d1[self.dist1_idx][self.g.pawn[1]]
        } else {
            self.d0[self.dist0_idx][self.g.pawn[0]]
        };
        let opp_player = if me == 0 { Player::Two } else { Player::One };

        let bridge = self.bridge.as_mut().expect("cat bridge");
        let cat = if depth >= 2 {
            bridge.bfs.build_corridor_attention(&bridge.board)
        } else {
            CorridorAttention::default()
        };
        let mut opp_path = [0u8; 81];
        let opp_path_len =
            get_shortest_path(&bridge.board, opp_player, &mut bridge.bfs, &mut opp_path);
        let reachable = bridge.bfs.both_reachable_mask(&bridge.board);
        let gap_zone = gap_play_zone_mask(reachable);

        for slot in 0..64 {
            for (wall_type, base) in [(0usize, 100i16), (1usize, 200i16)] {
                if !self.g.wall_fits(wall_type, slot) {
                    continue;
                }
                let m = base + slot as i16;
                let keep = m == tt_move
                    || wall_should_search(
                        ace_move_to_board(m),
                        &cat,
                        reachable,
                        gap_zone,
                        &mut bridge.board,
                        our_dist,
                        opp_dist,
                        &opp_path,
                        opp_path_len,
                        &mut bridge.bfs,
                    );
                if keep {
                    out[n] = m;
                    n += 1;
                }
            }
        }
        n
    }

    fn order_moves(&self, ply: usize, moves: &mut [i16], tt_move: i16, cm_move: i16) {
        let dist_me = if self.g.turn == 0 {
            &self.d0[self.dist0_idx]
        } else {
            &self.d1[self.dist1_idx]
        };
        let k = &self.killers[ply];
        let n = moves.len();
        let mut sc = [0i32; 160];
        for i in 0..n {
            let m = moves[i];
            sc[i] = if m == tt_move {
                2_000_000_000
            } else if m < 100 {
                1_000_000 - dist_me[m as usize] as i32 * 1000
            } else if m == k[0] {
                900_000
            } else if m == cm_move {
                870_000
            } else if m == k[1] {
                850_000
            } else {
                self.history_tbl[m as usize]
            };
        }
        // stable insertion sort, descending — must match JS tie order exactly
        for a in 1..n {
            let mv = moves[a];
            let ms = sc[a];
            let mut b = a as isize - 1;
            while b >= 0 && sc[b as usize] < ms {
                moves[(b + 1) as usize] = moves[b as usize];
                sc[(b + 1) as usize] = sc[b as usize];
                b -= 1;
            }
            moves[(b + 1) as usize] = mv;
            sc[(b + 1) as usize] = ms;
        }
    }

    /// True when the current board hash already appeared in real game history
    /// (since the last wall — same rule as the in-search repetition cutoff).
    fn repeats_game_history(&self) -> bool {
        let lwp = self.g.last_wall_ply as isize;
        let mut gi = self.g.hist_len as isize * 2 - 4;
        while gi >= lwp * 2 {
            if self.g.hashes_u[gi as usize] == self.g.hash_lo
                && self.g.hashes_u[gi as usize + 1] == self.g.hash_hi
            {
                return true;
            }
            gi -= 2;
        }
        false
    }

    fn move_repeats_game_history(&mut self, m: i16) -> bool {
        self.g.make_move(m);
        let rep = self.repeats_game_history();
        self.g.unmake_move();
        rep
    }

    fn ab(
        &mut self,
        depth: i32,
        mut alpha: i32,
        beta: i32,
        ply: usize,
        allow_null: bool,
        prev_move: i16,
    ) -> Result<i32, TimeUp> {
        self.nodes += 1;
        self.check_time()?;
        let prev = 1 - self.g.turn;
        if (prev == 0 && self.g.pawn[0] < 9) || (prev == 1 && self.g.pawn[1] >= 72) {
            return Ok(-(MATE - ply as i32));
        }
        if ply >= MAX_PLY - 1 {
            return Ok(0);
        }
        self.path_lo[ply] = self.g.hash_lo;
        self.path_hi[ply] = self.g.hash_hi;
        if ply > 0 {
            // repetition: search line, then game history back to last wall
            for ri in (0..ply).rev() {
                if self.path_lo[ri] == self.g.hash_lo && self.path_hi[ri] == self.g.hash_hi {
                    return Ok(0);
                }
            }
            let lwp = self.g.last_wall_ply as isize;
            let mut gi = self.g.hist_len as isize * 2 - 4;
            while gi >= lwp * 2 {
                if self.g.hashes_u[gi as usize] == self.g.hash_lo
                    && self.g.hashes_u[gi as usize + 1] == self.g.hash_hi
                {
                    return Ok(0);
                }
                gi -= 2;
            }
        }

        self.refresh_dist(ply);
        let nd0 = self.dist0_idx; // restored on every unmake
        let nd1 = self.dist1_idx;
        let nst = self.cached_stamp;
        if depth <= 0 {
            return Ok(self.evaluate());
        }

        // TT probe (typed, always-replace)
        let idx = (self.g.hash_lo & TT_MASK) as usize;
        let mut tt_move: i16 = 0;
        let meta = self.tt_meta[idx];
        if meta != 0
            && self.tt_key_hi[idx] == self.g.hash_hi
            && self.tt_key_lo[idx] == self.g.hash_lo
        {
            tt_move = (meta & 1023) as i16;
            let tdepth = meta >> 12;
            let tflag = (meta >> 10) & 3;
            if tdepth >= depth && ply > 0 {
                let mut es = self.tt_score[idx]; // mate scores stored node-relative
                if es > MATE - 2 * MAX_PLY as i32 {
                    es -= ply as i32;
                } else if es < -(MATE - 2 * MAX_PLY as i32) {
                    es += ply as i32;
                }
                if tflag == 0 {
                    return Ok(es);
                }
                if tflag == 1 && es >= beta {
                    return Ok(es);
                }
                if tflag == 2 && es <= alpha {
                    return Ok(es);
                }
            }
        }

        // reverse futility: hopeless to fall below beta at shallow depth
        if depth <= 4 && beta > -2000 && beta < 2000 {
            let sev = self.evaluate();
            if sev - 90 * depth >= beta {
                return Ok(sev);
            }
        }

        // null move
        if allow_null && depth >= 3 && ply > 0 {
            let ev = self.evaluate();
            if ev >= beta {
                let z = &ZOBRIST;
                self.g.turn ^= 1;
                self.g.hash_lo ^= z.turn_lo;
                self.g.hash_hi ^= z.turn_hi;
                if let Some(bridge) = self.bridge.as_mut() {
                    // keep the mirrored board's side in sync (wall accounting)
                    bridge.board.side_to_move = bridge.board.side_to_move.opposite();
                }
                let res = self.ab(depth - 3, -beta, -beta + 1, ply + 1, false, 0);
                let z = &ZOBRIST;
                self.g.turn ^= 1;
                self.g.hash_lo ^= z.turn_lo;
                self.g.hash_hi ^= z.turn_hi;
                if let Some(bridge) = self.bridge.as_mut() {
                    bridge.board.side_to_move = bridge.board.side_to_move.opposite();
                }
                self.dist0_idx = nd0;
                self.dist1_idx = nd1;
                self.cached_stamp = nst;
                let ns = -res?;
                if ns >= beta && ns < MATE - 200 {
                    return Ok(beta);
                }
            }
        }

        let mut moves = [0i16; 160];
        let n = self.gen_moves(ply, depth, tt_move, &mut moves);
        if n == 0 {
            return Ok(self.evaluate());
        }
        let cm_move = if prev_move > 0 {
            self.cm[prev_move as usize]
        } else {
            0
        };
        self.order_moves(ply, &mut moves[..n], tt_move, cm_move);

        let mut best = i32::MIN; // JS -Infinity
        let mut best_move: i16 = 0;
        let mut flag = 2;

        for i in 0..n {
            let m = moves[i];
            // frontier LMP
            if depth <= 2
                && ply > 0
                && i >= 10
                && m >= 100
                && m != tt_move
                && self.history_tbl[m as usize] <= 0
                && best > -MATE + 200
            {
                continue;
            }
            if m >= 100 && ply > 0 {
                let wt = if m < 200 { 0 } else { 1 };
                let slot = (m % 100) as usize;
                if self.g.wall_needs_path_check(wt, slot) {
                    self.g.set_wall_bits(wt, slot, true);
                    let paths_ok = self.g.has_path(0) && self.g.has_path(1);
                    self.g.set_wall_bits(wt, slot, false);
                    if !paths_ok {
                        continue; // sealing wall: pseudo-legal only
                    }
                }
            }
            self.g.make_move(m);
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.push(m);
            }
            let new_depth = depth - 1;
            let result = if i >= 4 && depth >= 3 && m >= 100 && m != tt_move {
                // graduated LMR
                let red =
                    1 + if i >= 12 { 1 } else { 0 } + if depth >= 6 && i >= 24 { 1 } else { 0 };
                let rd = (new_depth - red).max(0);
                match self.ab(rd, -alpha - 1, -alpha, ply + 1, true, m) {
                    Ok(s) => {
                        let mut score = -s;
                        if score > alpha {
                            match self.ab(new_depth, -beta, -alpha, ply + 1, true, m) {
                                Ok(s2) => score = -s2,
                                Err(e) => {
                                    self.unwind_move(nd0, nd1, nst);
                                    return Err(e);
                                }
                            }
                        }
                        Ok(score)
                    }
                    Err(e) => Err(e),
                }
            } else if i > 0 {
                match self.ab(new_depth, -alpha - 1, -alpha, ply + 1, true, m) {
                    Ok(s) => {
                        let mut score = -s;
                        if score > alpha && score < beta {
                            match self.ab(new_depth, -beta, -alpha, ply + 1, true, m) {
                                Ok(s2) => score = -s2,
                                Err(e) => {
                                    self.unwind_move(nd0, nd1, nst);
                                    return Err(e);
                                }
                            }
                        }
                        Ok(score)
                    }
                    Err(e) => Err(e),
                }
            } else {
                self.ab(new_depth, -beta, -alpha, ply + 1, true, m)
                    .map(|s| -s)
            };
            self.g.unmake_move();
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.pop();
            }
            self.dist0_idx = nd0;
            self.dist1_idx = nd1;
            self.cached_stamp = nst;
            let score = result?;

            let prefer_non_repeat = ply == 0
                && score == best
                && best_move != 0
                && self.move_repeats_game_history(best_move)
                && !self.move_repeats_game_history(m);

            if score > best || prefer_non_repeat {
                best = score;
                best_move = m;
                if score > alpha || prefer_non_repeat {
                    alpha = score;
                    flag = 0;
                    if ply == 0 {
                        self.root_best = m;
                        self.root_score = score;
                    }
                    if alpha >= beta {
                        flag = 1;
                        if m >= 100 {
                            if self.killers[ply][0] != m {
                                self.killers[ply][1] = self.killers[ply][0];
                                self.killers[ply][0] = m;
                            }
                            self.history_tbl[m as usize] += depth * depth;
                            if self.history_tbl[m as usize] > 100_000_000 {
                                for h in self.history_tbl.iter_mut() {
                                    *h >>= 1;
                                }
                            }
                        }
                        if prev_move > 0 {
                            self.cm[prev_move as usize] = m;
                        }
                        break;
                    }
                }
            }
        }

        if best == i32::MIN {
            return Ok(self.evaluate()); // all pseudo-legal moves were sealing walls
        }
        let mut ts = best; // store mate scores node-relative
        if ts > MATE - 2 * MAX_PLY as i32 {
            ts += ply as i32;
        } else if ts < -(MATE - 2 * MAX_PLY as i32) {
            ts -= ply as i32;
        }
        self.tt_key_hi[idx] = self.g.hash_hi;
        self.tt_key_lo[idx] = self.g.hash_lo;
        self.tt_meta[idx] = best_move as i32 | (flag << 10) | (depth << 12);
        self.tt_score[idx] = ts;
        Ok(best)
    }

    /// Restore after a time abort mid-move (JS `finally` semantics).
    fn unwind_move(&mut self, nd0: usize, nd1: usize, nst: i32) {
        self.g.unmake_move();
        if let Some(bridge) = self.bridge.as_mut() {
            bridge.pop();
        }
        self.dist0_idx = nd0;
        self.dist1_idx = nd1;
        self.cached_stamp = nst;
    }

    // ── Rollout verification (minimax-MCTS hybrid) ──────────────────────────
    //
    // Between ID depths (≥6) the current best move is rolled along the search
    // PV then continued to terminal with Gorisanson + CAT guidance. αβ then
    // tries to refute that line; if the rollout is cut completely we retry
    // with a different stochastic branch until αβ accepts the path or time
    // runs out. Only AB-validated rollouts gate the easy-move stop.

    fn tt_best_move(&self) -> i16 {
        let idx = (self.g.hash_lo & TT_MASK) as usize;
        if self.tt_meta[idx] != 0
            && self.tt_key_hi[idx] == self.g.hash_hi
            && self.tt_key_lo[idx] == self.g.hash_lo
        {
            (self.tt_meta[idx] & 1023) as i16
        } else {
            0
        }
    }

    /// Best sequence from search TT: forced root move + PV continuation.
    fn extract_search_pv(&mut self, root_move: i16, max_plies: usize) -> Vec<i16> {
        let mut pv = vec![root_move];
        let nd0 = self.dist0_idx;
        let nd1 = self.dist1_idx;
        let nst = self.cached_stamp;

        self.g.make_move(root_move);
        if let Some(bridge) = self.bridge.as_mut() {
            bridge.push(root_move);
        }

        for _ in 1..max_plies {
            if self.g.winner() >= 0 {
                break;
            }
            let m = self.tt_best_move();
            if m == 0 {
                break;
            }
            let mut legal = [0i16; 160];
            let n = self.gen_moves(0, 1, 0, &mut legal);
            if !legal[..n].contains(&m) {
                break;
            }
            pv.push(m);
            self.g.make_move(m);
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.push(m);
            }
        }

        for _ in 0..pv.len() {
            self.g.unmake_move();
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.pop();
            }
        }
        self.dist0_idx = nd0;
        self.dist1_idx = nd1;
        self.cached_stamp = nst;
        pv
    }

    fn leaf_score_root_pov(&mut self, root_side: usize, ply: usize) -> i32 {
        let w = self.g.winner();
        if w >= 0 {
            if w as usize == root_side {
                MATE - ply as i32
            } else {
                -(MATE - ply as i32)
            }
        } else {
            self.refresh_dist(ply);
            let ev = self.evaluate();
            if self.g.turn == root_side {
                ev
            } else {
                -ev
            }
        }
    }

    /// Gorisanson shortest-path step via mirrored Titanium board (80% branch).
    fn gori_bridge_step(
        bridge: &mut TiBridge,
        rng: &mut u64,
        next_p1: &mut [u8; 81],
        next_p2: &mut [u8; 81],
        p1_valid: &mut bool,
        p2_valid: &mut bool,
        cat: Option<&CorridorAttention>,
    ) -> Option<i16> {
        let board = &mut bridge.board;
        let stm = board.side();
        if !*p1_valid {
            bridge
                .bfs
                .fill_next_toward_goal(board, Player::One, next_p1);
            *p1_valid = true;
        }
        if !*p2_valid {
            bridge
                .bfs
                .fill_next_toward_goal(board, Player::Two, next_p2);
            *p2_valid = true;
        }

        if next_rand(rng) % 10 < 8 {
            let (pr, pc) = board.pawn(stm);
            let sq = square_index(pr, pc);
            let next_sq = if stm == Player::One {
                next_p1[sq as usize]
            } else {
                next_p2[sq as usize]
            };
            if next_sq != u8::MAX {
                let (nr, nc) = unpack_square(next_sq);
                return Some(board_move_to_ace(BoardMove::Pawn { row: nr, col: nc }));
            }
        }

        let mut legal = [BoardMove::Pawn { row: 0, col: 0 }; MAX_LEGAL_MOVES];
        let n = generate_legal_moves_slice(board, &mut legal, &mut bridge.bfs);
        if n == 0 {
            return None;
        }
        let mv = if let Some(cat) = cat {
            pick_hot_rollout_board(&legal, n, cat, rng)
        } else {
            legal[(next_rand(rng) as usize) % n]
        };
        if matches!(mv, BoardMove::Wall { .. }) {
            *p1_valid = false;
            *p2_valid = false;
        }
        Some(board_move_to_ace(mv))
    }

    /// Gorisanson + CAT when bridge exists; ACE greedy policy otherwise.
    fn rollout_policy_move_guided(
        &mut self,
        rng: &mut u64,
        cat: Option<&CorridorAttention>,
        next_p1: &mut [u8; 81],
        next_p2: &mut [u8; 81],
        p1_valid: &mut bool,
        p2_valid: &mut bool,
    ) -> i16 {
        if let Some(bridge) = self.bridge.as_mut() {
            if let Some(m) =
                Self::gori_bridge_step(bridge, rng, next_p1, next_p2, p1_valid, p2_valid, cat)
            {
                return m;
            }
        }
        self.rollout_policy_move(rng)
    }

    /// Whether a rollout checkpoint is worth the time (minimax keeps priority).
    fn rollout_checkpoint_warranted(
        d: i32,
        stable: i32,
        last_best: i16,
        last_score: i32,
        ver_passed_move: i16,
        rollout_done: bool,
        elapsed_ms: u64,
        time_ms: u64,
        g: &AceGame,
    ) -> bool {
        if rollout_done || last_best == 0 || ver_passed_move == last_best {
            return false;
        }
        if d < 6 || stable < 2 {
            return false;
        }
        if last_score > MATE - 500 || last_score < -(MATE - 500) {
            return false;
        }
        if elapsed_ms.saturating_mul(100) > time_ms.saturating_mul(75) {
            return false;
        }
        let walls_left = g.wl[0] + g.wl[1];
        if walls_left <= 4 {
            return false;
        }
        true
    }

    /// Play PV prefix then a bounded guided continuation (not always to terminal).
    fn rollout_pv_to_terminal(
        &mut self,
        pv_prefix: &[i16],
        root_side: usize,
        rng: &mut u64,
        max_extra_guided: usize,
    ) -> Option<(Vec<i16>, i32, bool)> {
        let cap = (pv_prefix.len() + max_extra_guided)
            .min(48)
            .min(200usize.min(1000usize.saturating_sub(self.g.hist_len)));
        let nd0 = self.dist0_idx;
        let nd1 = self.dist1_idx;
        let nst = self.cached_stamp;
        let mut reached_terminal = false;
        let mut spine = Vec::with_capacity(cap);
        let mut next_p1 = [u8::MAX; 81];
        let mut next_p2 = [u8::MAX; 81];
        let mut p1_valid = false;
        let mut p2_valid = false;
        let mut cat: Option<CorridorAttention> = None;

        for &m in pv_prefix {
            if spine.len() >= cap {
                break;
            }
            self.g.make_move(m);
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.push(m);
                if matches!(ace_move_to_board(m), BoardMove::Wall { .. }) {
                    p1_valid = false;
                    p2_valid = false;
                }
            }
            spine.push(m);
            if self.g.winner() >= 0 {
                reached_terminal = true;
                break;
            }
        }
        let mut guided = 0usize;

        loop {
            if self.g.winner() >= 0 {
                reached_terminal = true;
                break;
            }
            if spine.len() >= cap {
                break;
            }
            if guided >= max_extra_guided {
                break;
            }
            if Instant::now() > self.deadline {
                for _ in 0..spine.len() {
                    self.g.unmake_move();
                    if let Some(bridge) = self.bridge.as_mut() {
                        bridge.pop();
                    }
                }
                self.dist0_idx = nd0;
                self.dist1_idx = nd1;
                self.cached_stamp = nst;
                return None;
            }
            if self.bridge.is_some() && cat.is_none() {
                if let Some(bridge) = self.bridge.as_mut() {
                    cat = Some(build_corridor_attention(&mut bridge.bfs, &bridge.board));
                }
            }
            let m = self.rollout_policy_move_guided(
                rng,
                cat.as_ref(),
                &mut next_p1,
                &mut next_p2,
                &mut p1_valid,
                &mut p2_valid,
            );
            if m == 0 {
                break;
            }
            self.g.make_move(m);
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.push(m);
                if matches!(ace_move_to_board(m), BoardMove::Wall { .. }) {
                    p1_valid = false;
                    p2_valid = false;
                    cat = None;
                }
            }
            spine.push(m);
            guided += 1;
        }

        let leaf = self.leaf_score_root_pov(root_side, spine.len());
        for _ in 0..spine.len() {
            self.g.unmake_move();
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.pop();
            }
        }
        self.dist0_idx = nd0;
        self.dist1_idx = nd1;
        self.cached_stamp = nst;
        Some((spine, leaf, reached_terminal))
    }

    /// αβ trap check: walk the search PV, then 10-ply null-window proof from the
    /// leaf — catches wall blocks deep in the expected line (d9 blind spots).
    fn rollout_survives_ab(&mut self, pv: &[i16], root_side: usize, search_score: i32) -> bool {
        if pv.is_empty() {
            return false;
        }
        let margin = 70;
        let floor = search_score.saturating_sub(margin);
        let nd0 = self.dist0_idx;
        let nd1 = self.dist1_idx;
        let nst = self.cached_stamp;
        let mut played = 0usize;

        for &m in pv {
            if self.g.winner() >= 0 {
                break;
            }
            self.g.make_move(m);
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.push(m);
            }
            played += 1;
        }

        let survived = if self.g.winner() >= 0 {
            self.g.winner() as usize == root_side
        } else {
            let ply = played;
            let prev = pv[played.saturating_sub(1)];
            self.refresh_dist(ply);
            match self.ab(TRAP_PROOF_PLIES, floor - 1, floor, ply, true, prev) {
                Ok(score_stm) => {
                    let root_pov = if self.g.turn == root_side {
                        score_stm
                    } else {
                        -score_stm
                    };
                    root_pov >= floor
                }
                Err(_) => false,
            }
        };

        for _ in 0..played {
            self.g.unmake_move();
            if let Some(bridge) = self.bridge.as_mut() {
                bridge.pop();
            }
        }
        self.dist0_idx = nd0;
        self.dist1_idx = nd1;
        self.cached_stamp = nst;
        survived
    }

    /// Greedy playout policy with light randomization: run along the shortest
    /// path, or place a wall whose opponent-detour beats the race tempo.
    /// Considers only walls cutting an opponent shortest-path edge.
    fn rollout_policy_move(&mut self, rng: &mut u64) -> i16 {
        let me = self.g.turn;
        let opp = 1 - me;
        let mut dm = [0u8; 81];
        let mut dop = [0u8; 81];
        self.g.compute_dist(me, &mut dm);
        self.g.compute_dist(opp, &mut dop);
        let my_d = dm[self.g.pawn[me]] as i32;
        let opp_d = dop[self.g.pawn[opp]] as i32;

        // candidate "lead after my turn" scores: pawn = opp_d - dist(target),
        // wall = new_opp_d - new_my_d (tempo cost implicit: the pawn stands still)
        let mut best_m: i16 = 0;
        let mut best_s = i32::MIN;
        let mut second_m: i16 = 0;
        let mut second_s = i32::MIN;

        let mut pawn = [0i16; 16];
        let pn = self.g.gen_pawn_moves(&mut pawn, 0);
        let opp_cell = self.g.pawn[opp] as i16;
        for &p in &pawn[..pn] {
            let mut s = opp_d - dm[p as usize] as i32;
            // face-off tempo: stepping adjacent to the opponent gifts them a
            // jump over us if the landing square advances them — dist can't
            // see jumps, so charge the stolen tempo here
            let diff = p - opp_cell;
            if diff == 1 || diff == -1 || diff == 9 || diff == -9 {
                // they jump from their cell over us, landing beyond us
                let jump = p + diff;
                let dir = match diff {
                    -9 => 0,
                    9 => 1,
                    -1 => 2,
                    _ => 3,
                };
                if (0..81).contains(&jump)
                    && self.g.can_step(p as usize, dir)
                    && (dop[jump as usize] as i32) < dop[opp_cell as usize] as i32 - 1
                {
                    s -= 1;
                }
            }
            if s > best_s {
                second_m = best_m;
                second_s = best_s;
                best_m = p;
                best_s = s;
            } else if s > second_s {
                second_m = p;
                second_s = s;
            }
        }
        // a wall spends a scarce resource without advancing the pawn: it must
        // strictly beat stepping, or greedy play degenerates into wall-spam
        let wall_floor = best_s + 1;

        if self.g.wl[me] > 0 {
            for slot in 0..64usize {
                let a = (slot >> 3) * 9 + (slot & 7);
                for wt in 0..2usize {
                    if !self.g.wall_fits(wt, slot) {
                        continue;
                    }
                    // relevance: the wall must cut an opponent shortest-path edge
                    let (e1a, e1b, e2a, e2b) = if wt == 0 {
                        (a, a + 9, a + 1, a + 10) // hw blocks two vertical edges
                    } else {
                        (a, a + 1, a + 9, a + 10) // vw blocks two horizontal edges
                    };
                    let cuts = dop[e1a].abs_diff(dop[e1b]) == 1 || dop[e2a].abs_diff(dop[e2b]) == 1;
                    if !cuts {
                        continue;
                    }
                    let needs = self.g.wall_needs_path_check(wt, slot);
                    self.g.set_wall_bits(wt, slot, true);
                    let ok = !needs || (self.g.has_path(0) && self.g.has_path(1));
                    if ok {
                        let mut nd_op = [0u8; 81];
                        let mut nd_me = [0u8; 81];
                        self.g.compute_dist(opp, &mut nd_op);
                        self.g.compute_dist(me, &mut nd_me);
                        let new_opp_d = nd_op[self.g.pawn[opp]] as i32;
                        let new_my_d = nd_me[self.g.pawn[me]] as i32;
                        let s = (new_opp_d - new_my_d) - (new_my_d - my_d).max(0);
                        if s >= wall_floor && s > best_s {
                            second_m = best_m;
                            second_s = best_s;
                            best_m = (if wt == 0 { 100 } else { 200 }) + slot as i16;
                            best_s = s;
                        } else if s >= wall_floor && s > second_s {
                            second_m = (if wt == 0 { 100 } else { 200 }) + slot as i16;
                            second_s = s;
                        }
                    }
                    self.g.set_wall_bits(wt, slot, false);
                }
            }
        }

        // Gorisanson-style randomization: 70% follow greedy best move,
        // 30% take a random move to explore variation and avoid overfit
        if next_rand(rng) % 100 >= 70 {
            // 30% of the time: pick a random legal move instead of best_m
            let mut moves = [0i16; 160];
            let n = self.gen_moves(0, 1, 0, &mut moves);
            if n > 0 {
                return moves[(next_rand(rng) as usize) % n];
            }
        }
        best_m
    }

    /// Ply-cap adjudication: side to move wins ties (it has the tempo).
    fn rollout_adjudicate(&mut self, root_side: usize) -> bool {
        let me = self.g.turn;
        let opp = 1 - me;
        let mut dm = [0u8; 81];
        let mut dop = [0u8; 81];
        self.g.compute_dist(me, &mut dm);
        self.g.compute_dist(opp, &mut dop);
        let winner = if (dm[self.g.pawn[me]] as i32) <= dop[self.g.pawn[opp]] as i32 {
            me
        } else {
            opp
        };
        winner == root_side
    }

    /// Entry: iterative deepening within `time_ms`. `full` disables the easy-move stop.
    pub fn think(
        &mut self,
        time_ms: u64,
        max_depth: i32,
        full: bool,
        log: bool,
        engine_label: &str,
    ) -> ThinkResult {
        let t0 = Instant::now();
        self.deadline = t0 + Duration::from_millis(time_ms);
        self.nodes = 0;
        self.root_best = 0;
        self.root_score = 0;
        let mut last_best: i16 = 0;
        let mut last_score = 0;
        let mut last_depth = 0;
        let mut stable = 0;
        let mut depth_log: Vec<AceDepthLogEntry> = Vec::new();
        let max_depth = if max_depth > 0 { max_depth } else { 30 };

        // Rollout verification: once per think when the root move is stable (≥2
        // depths) at d≥6, roll a short PV continuation and run a cheap αβ check.
        // Skipped in endgame / low time so minimax keeps most of the budget.
        let mut ver_move: i16 = 0;
        let mut ver_passed_move: i16 = 0;
        let mut ver_attempts: u32 = 0;
        let mut ver_spine_plies: u32 = 0;
        let mut ver_leaf_score: i32 = 0;
        let mut ver_confirmed = false;
        let mut ver_suspect = false;
        let mut rollout_done_this_think = false;
        // position-seeded → deterministic per position, varied across the game
        let mut rng: u64 =
            (((self.g.hash_hi as u64) << 32) | self.g.hash_lo as u64) ^ 0x9E37_79B9_7F4A_7C15 | 1;

        for d in 1..=max_depth {
            let nodes_at_depth = self.nodes;
            let result = if d >= 4 && last_score > -2000 && last_score < 2000 {
                // aspiration
                let mut lo = last_score - 75;
                let mut hi = last_score + 75;
                loop {
                    match self.ab(d, lo, hi, 0, true, 0) {
                        Ok(sc) => {
                            if sc <= lo {
                                lo = -INF;
                            } else if sc >= hi {
                                hi = INF;
                            } else {
                                break Ok(sc);
                            }
                        }
                        Err(e) => break Err(e),
                    }
                }
            } else {
                self.ab(d, -INF, INF, 0, true, 0)
            };
            match result {
                Ok(sc) => {
                    stable = if self.root_best == last_best {
                        stable + 1
                    } else {
                        0
                    };
                    last_best = self.root_best;
                    last_score = sc;
                    last_depth = d;
                    let elapsed_ms = t0.elapsed().as_millis() as u64;
                    let pv = if last_best != 0 {
                        super::ace_to_algebraic(last_best)
                    } else {
                        String::new()
                    };
                    depth_log.push(AceDepthLogEntry {
                        depth: d,
                        score: last_score,
                        nodes: self.nodes,
                        elapsed_ms,
                        marginal_nodes: self.nodes.saturating_sub(nodes_at_depth),
                        pv,
                    });
                    if log {
                        self.refresh_dist(0);
                        let white_dist = self.d0[self.dist0_idx][self.g.pawn[0]];
                        let black_dist = self.d1[self.dist1_idx][self.g.pawn[1]];
                        emit_ace_progress(
                            engine_label,
                            &depth_log,
                            d,
                            self.nodes,
                            last_score,
                            white_dist,
                            black_dist,
                            elapsed_ms,
                        );
                    }
                    if sc > MATE - 200 || sc < -(MATE - 200) {
                        break; // forced result
                    }
                    // Rollout checkpoint: at most once per think, stable move only.
                    let elapsed_ms = t0.elapsed().as_millis() as u64;
                    if self.pseudo_mcts
                        && Self::rollout_checkpoint_warranted(
                            d,
                            stable,
                            last_best,
                            last_score,
                            ver_passed_move,
                            rollout_done_this_think,
                            elapsed_ms,
                            time_ms,
                            &self.g,
                        )
                    {
                        rollout_done_this_think = true;
                        ver_move = last_best;
                        ver_attempts = 0;
                        ver_confirmed = false;
                        ver_suspect = false;
                        let saved_deadline = self.deadline;
                        let remaining_ms = self
                            .deadline
                            .saturating_duration_since(Instant::now())
                            .as_millis() as u64;
                        let batch_ms = (remaining_ms / 5).min(time_ms / 10).max(50).min(200);
                        if batch_ms >= 60 {
                            self.deadline = (Instant::now() + Duration::from_millis(batch_ms))
                                .min(saved_deadline);
                            let root_side = self.g.turn;
                            let pv = self.extract_search_pv(last_best, d as usize + 4);
                            let max_extra =
                                ((d as usize) / 2).max(TRAP_PROOF_PLIES as usize).min(16);
                            let max_tries = if remaining_ms < time_ms / 3 { 2 } else { 3 };
                            loop {
                                if Instant::now() >= self.deadline || ver_attempts >= max_tries {
                                    break;
                                }
                                ver_attempts += 1;
                                let Some((spine, leaf, _)) = self
                                    .rollout_pv_to_terminal(&pv, root_side, &mut rng, max_extra)
                                else {
                                    break;
                                };
                                ver_spine_plies = spine.len() as u32;
                                ver_leaf_score = leaf;
                                if self.rollout_survives_ab(&pv, root_side, last_score) {
                                    ver_confirmed = true;
                                    ver_passed_move = last_best;
                                    ver_suspect = false;
                                    break;
                                }
                                ver_suspect = true;
                                rng ^= next_rand(&mut rng) | 1;
                            }
                        } else {
                            ver_passed_move = last_best;
                        }
                        self.deadline = saved_deadline;
                        if log && ver_attempts > 0 {
                            let verdict = if ver_confirmed {
                                "confirmed-ab"
                            } else if ver_suspect {
                                "refuted-retry-exhausted"
                            } else {
                                "aborted"
                            };
                            emit_rollout_stats(
                                engine_label,
                                ver_move,
                                ver_attempts,
                                ver_spine_plies,
                                ver_leaf_score,
                                last_score,
                                verdict,
                            );
                        }
                    }
                    // v8 easy-move stop (acev8_engine.js) — with pseudo-MCTS the
                    // move must also be rollout-confirmed before banking time
                    if !full
                        && d >= 9
                        && stable >= 3
                        && last_score > -120
                        && (!self.pseudo_mcts
                            || ver_confirmed
                            || ver_passed_move == last_best
                            || !rollout_done_this_think)
                        && t0.elapsed().as_millis() as u64 > time_ms * 3 / 10
                    {
                        break;
                    }
                }
                Err(TimeUp) => break, // state already restored by unwinding unmakes
            }
            // suspect main move: stretch the budget so αβ must prove or reject it
            let time_frac = if ver_suspect {
                0.97
            } else if last_score < -80 {
                0.92
            } else {
                0.85
            };
            if t0.elapsed().as_millis() as f64 > time_ms as f64 * time_frac {
                break;
            }
        }

        if last_best == 0 {
            self.refresh_dist(0);
            let mut moves = [0i16; 160];
            let n = self.gen_moves(0, 1, 0, &mut moves);
            if n > 0 {
                last_best = moves[0];
            }
        }

        if self.pseudo_mcts && ver_move != 0 && ver_move == last_best && ver_attempts > 0 {
            let verdict = if ver_confirmed {
                "confirmed-played"
            } else if ver_suspect {
                "suspect-played-anyway"
            } else {
                "inconclusive"
            };
            emit_rollout_stats(
                engine_label,
                ver_move,
                ver_attempts,
                ver_spine_plies,
                ver_leaf_score,
                last_score,
                verdict,
            );
        }

        self.refresh_dist(0);
        let white_dist = self.d0[self.dist0_idx][self.g.pawn[0]];
        let black_dist = self.d1[self.dist1_idx][self.g.pawn[1]];
        let ms = t0.elapsed().as_millis() as u64;

        ThinkResult {
            mv: last_best,
            score: last_score,
            depth: last_depth,
            nodes: self.nodes,
            ms,
            white_dist,
            black_dist,
            depth_log,
        }
    }
}

#[cfg(test)]
mod rollout_tests {
    use super::*;

    /// Dump one policy game to inspect degenerate play patterns.
    #[test]
    #[ignore]
    fn rollout_policy_dump_game() {
        let mut s = AceSearch::new(AceGame::new());
        s.deadline = Instant::now() + Duration::from_secs(60);
        let mut rng: u64 = 0xDEAD_BEEF_1234_5677 | 1;
        let mut made = 0usize;
        let mut line = String::new();
        loop {
            let w = s.g.winner();
            if w >= 0 {
                println!("winner: p{}", w);
                break;
            }
            if made >= 300 {
                println!("cap hit");
                break;
            }
            let m = s.rollout_policy_move(&mut rng);
            if m == 0 {
                println!("no move");
                break;
            }
            line.push_str(&super::super::ace_to_algebraic(m));
            line.push(' ');
            s.g.make_move(m);
            made += 1;
        }
        println!("game ({} plies): {}", made, line);
        for _ in 0..made {
            s.g.unmake_move();
        }
    }

    #[test]
    fn extract_search_pv_restores_position() {
        let mut s = AceSearch::with_ti_movegen(AceGame::new());
        s.enable_pseudo_mcts();
        s.g.make_move(super::super::algebraic_to_ace("c3h"));
        s.position_changed();
        s.g.make_move(super::super::algebraic_to_ace("e8"));
        s.position_changed();
        let hash_lo = s.g.hash_lo;
        let hash_hi = s.g.hash_hi;
        let turn = s.g.turn;
        let pawn0 = s.g.pawn[0];
        let pawn1 = s.g.pawn[1];
        let _ = s.extract_search_pv(super::super::algebraic_to_ace("e2"), 8);
        assert_eq!(s.g.hash_lo, hash_lo);
        assert_eq!(s.g.hash_hi, hash_hi);
        assert_eq!(s.g.turn, turn);
        assert_eq!(s.g.pawn[0], pawn0);
        assert_eq!(s.g.pawn[1], pawn1);
        if let Some(bridge) = s.bridge.as_ref() {
            assert_eq!(bridge.undo_stack.len(), 0);
        }
    }

    /// Pure policy self-play from startpos must be near 50/50 — a skewed
    /// winner split means the playout policy (not the position) decides games.
    #[test]
    #[ignore] // diagnostic, run with -- --ignored --nocapture
    fn rollout_policy_selfplay_balance() {
        let mut s = AceSearch::new(AceGame::new());
        s.deadline = Instant::now() + Duration::from_secs(60);
        let mut rng: u64 = 0x1234_5678_9ABC_DEF0 | 1;
        let mut p0_wins = 0u32;
        let mut games = 0u32;
        let mut lens = 0u64;
        for _ in 0..200 {
            // play a full policy game from the root (both sides policy)
            let mut made = 0usize;
            let win: i32;
            loop {
                let w = s.g.winner();
                if w >= 0 {
                    win = w;
                    break;
                }
                if made >= 300 {
                    win = -1;
                    break;
                }
                let m = s.rollout_policy_move(&mut rng);
                if m == 0 {
                    win = -1;
                    break;
                }
                s.g.make_move(m);
                made += 1;
            }
            lens += made as u64;
            for _ in 0..made {
                s.g.unmake_move();
            }
            if win == 0 {
                p0_wins += 1;
            }
            if win >= 0 {
                games += 1;
            }
        }
        println!(
            "policy selfplay: games={} p0_wins={} ({:.0}%), avg len={}",
            games,
            p0_wins,
            100.0 * p0_wins as f64 / games.max(1) as f64,
            lens / 200
        );
    }
}
                                                                                                                                                                                                                                                                                                                                                                       