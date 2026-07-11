//! Experimental port of the "Ka" (epoch15000.ckpt) policy head from
//! reference/ace.html's ka_ab.js / ka_forward.js, for A/B-testing whether a
//! learned 137-way move prior improves Titanium's move ordering.
//!
//! Weights are NOT committed to the repo (reference/ace.html and its export
//! under reference/ka_weights_export/ are gitignored — provenance/license of
//! the third-party trained checkpoint is unclear). This module loads the
//! self-describing binary produced by the export script at RUNTIME from a
//! path, and is a no-op (returns `None` from `load`) if the file is absent —
//! default builds and the shipped site are completely unaffected.
//!
//! Value head is intentionally NOT ported (scoped decision: ordering-prior
//! experiment first, LMR/eval untouched).
//!
//! Architecture (from the exported meta): 18-layer trunk, 128 channels,
//! softsign layernorm, self-attention at layers {3,6,9,12,15} (4 heads x 32),
//! residual conv elsewhere (3x3 for layers <=5, 1x1 beyond); policy head:
//! conv 128->32 (3x3, ReLU) -> flatten (81*32=2592) -> dense -> 137-way
//! softmax over [128 wall placements (64 h + 64 v), 9 pawn directions].

use crate::titanium::game::GameState;
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::Path;

const TRUNK_LAYERS: usize = 18;
const ATTN_LAYERS: [usize; 5] = [3, 6, 9, 12, 15];
const C: usize = 128;
const CELLS: usize = 81;
/// Q/K weights below this magnitude only produce effectively uniform attention
/// for the exported Ka checkpoint. Keep this deliberately conservative: a
/// layer is elided only when both projections are within this bound.
const UNIFORM_ATTENTION_QK_MAX_ABS: f32 = 1e-5;

pub struct KaPolicyNet {
    // layer 0
    w_conv0: Vec<f32>,    // [3,3,15,128]
    beta: Vec<Vec<f32>>,  // [18][128]
    gamma: Vec<Vec<f32>>, // [18][128]
    // conv-residual layers (i in 1..18, i%3!=0): kernel 3 for i<=5 else 1
    w_conv: HashMap<usize, Vec<f32>>,
    // attention layers
    // Q/K are intentionally absent for `uniform_attention` layers.
    wq: HashMap<usize, Vec<f32>>,
    wk: HashMap<usize, Vec<f32>>,
    wv: HashMap<usize, Vec<f32>>,
    uniform_attention: [bool; TRUNK_LAYERS],
    // policy head
    w_p_head_conv: Vec<f32>, // [3,3,128,32]
    b_p_head_conv: Vec<f32>, // [32]
    w2: Vec<f32>,            // [2592,137]
    b2: Vec<f32>,            // [137]
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_f32_vec(r: &mut impl Read, n: usize) -> io::Result<Vec<f32>> {
    let mut buf = vec![0u8; n * 4];
    r.read_exact(&mut buf)?;
    Ok(buf
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// Whether zeroing Q/K changes attention only by negligible logits. Non-finite
/// values are kept on the generic path rather than being silently discarded.
fn has_effectively_uniform_attention(q: &[f32], k: &[f32]) -> bool {
    fn max_abs_within_threshold(weights: &[f32]) -> bool {
        weights
            .iter()
            .all(|&weight| weight.is_finite() && weight.abs() <= UNIFORM_ATTENTION_QK_MAX_ABS)
    }

    max_abs_within_threshold(q) && max_abs_within_threshold(k)
}

impl KaPolicyNet {
    /// Loads the self-describing binary written by
    /// reference/ka_weights_export's exporter. Returns `Ok(None)` (not an
    /// error) if `path` doesn't exist, so callers can treat this feature as
    /// silently unavailable outside the author's own machine.
    pub fn load(path: &Path) -> io::Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let mut f = std::fs::File::open(path)?;
        let n_tensors = read_u32(&mut f)? as usize;

        let mut beta = vec![Vec::new(); TRUNK_LAYERS];
        let mut gamma = vec![Vec::new(); TRUNK_LAYERS];
        let mut w_conv0 = Vec::new();
        let mut w_conv = HashMap::new();
        let mut wq = HashMap::new();
        let mut wk = HashMap::new();
        let mut wv = HashMap::new();
        let mut w_p_head_conv = Vec::new();
        let mut b_p_head_conv = Vec::new();
        let mut w2 = Vec::new();
        let mut b2 = Vec::new();

        for _ in 0..n_tensors {
            let name_len = read_u32(&mut f)? as usize;
            let mut name_buf = vec![0u8; name_len];
            f.read_exact(&mut name_buf)?;
            let name = String::from_utf8_lossy(&name_buf).to_string();
            let ndim = read_u32(&mut f)? as usize;
            for _ in 0..ndim {
                let _ = read_u32(&mut f)?;
            }
            let count = read_u32(&mut f)? as usize;
            let data = read_f32_vec(&mut f, count)?;

            if name == "W_convs.0" {
                w_conv0 = data;
            } else if let Some(rest) = name.strip_prefix("betas.") {
                let idx: usize = rest.parse().unwrap_or(usize::MAX);
                if idx < TRUNK_LAYERS {
                    beta[idx] = data;
                }
            } else if let Some(rest) = name.strip_prefix("gammas.") {
                let idx: usize = rest.parse().unwrap_or(usize::MAX);
                if idx < TRUNK_LAYERS {
                    gamma[idx] = data;
                }
            } else if let Some(rest) = name.strip_prefix("W_convs.") {
                let idx: usize = rest.parse().unwrap_or(usize::MAX);
                if idx > 0 && idx < TRUNK_LAYERS && idx % 3 != 0 {
                    w_conv.insert(idx, data);
                }
                // idx % 3 == 0 (attention layers): dead weight, discarded.
            } else if let Some(rest) = name.strip_prefix("WQs.") {
                let idx: usize = rest.parse().unwrap_or(usize::MAX);
                if ATTN_LAYERS.contains(&idx) {
                    wq.insert(idx, data);
                }
            } else if let Some(rest) = name.strip_prefix("WKs.") {
                let idx: usize = rest.parse().unwrap_or(usize::MAX);
                if ATTN_LAYERS.contains(&idx) {
                    wk.insert(idx, data);
                }
            } else if let Some(rest) = name.strip_prefix("WVs.") {
                let idx: usize = rest.parse().unwrap_or(usize::MAX);
                if ATTN_LAYERS.contains(&idx) {
                    wv.insert(idx, data);
                }
            } else if name == "W_p_head_conv" {
                w_p_head_conv = data;
            } else if name == "b_p_head_conv" {
                b_p_head_conv = data;
            } else if name == "W2" {
                w2 = data;
            } else if name == "b2" {
                b2 = data;
            }
        }

        // The checkpoint's attention Q/K projections are near zero. Once both
        // are known, drop them entirely and use exact uniform attention over V
        // at inference time. This also makes the saved network materially
        // smaller instead of merely skipping the corresponding matmuls.
        let mut uniform_attention = [false; TRUNK_LAYERS];
        for &layer in &ATTN_LAYERS {
            let should_prune = match (wq.get(&layer), wk.get(&layer)) {
                (Some(q), Some(k)) => has_effectively_uniform_attention(q, k),
                _ => false,
            };
            if should_prune {
                uniform_attention[layer] = true;
                wq.remove(&layer);
                wk.remove(&layer);
            }
        }

        Ok(Some(Self {
            w_conv0,
            beta,
            gamma,
            w_conv,
            wq,
            wk,
            wv,
            uniform_attention,
            w_p_head_conv,
            b_p_head_conv,
            w2,
            b2,
        }))
    }

    /// SAME conv, stride 1, no bias. x: [81*cin], w: [(k*k*cin)*cout] HWIO.
    fn conv(x: &[f32], cin: usize, cout: usize, w: &[f32], k: usize, out: &mut [f32]) {
        out.iter_mut().for_each(|v| *v = 0.0);
        let r = (k as i32 - 1) >> 1;
        for ix in 0..9i32 {
            for iy in 0..9i32 {
                let o_base = ((ix * 9 + iy) as usize) * cout;
                for dx in -r..=r {
                    let jx = ix + dx;
                    if jx < 0 || jx > 8 {
                        continue;
                    }
                    for dy in -r..=r {
                        let jy = iy + dy;
                        if jy < 0 || jy > 8 {
                            continue;
                        }
                        let x_base = ((jx * 9 + jy) as usize) * cin;
                        let w_base = (((dx + r) as usize * k + (dy + r) as usize) * cin) * cout;
                        for ci in 0..cin {
                            let xv = x[x_base + ci];
                            if xv == 0.0 {
                                continue;
                            }
                            let w_row = w_base + ci * cout;
                            for co in 0..cout {
                                out[o_base + co] += xv * w[w_row + co];
                            }
                        }
                    }
                }
            }
        }
    }

    /// Scalar-mean softsign layernorm (FORWARD_SPEC section 2).
    fn norm(x: &[f32], ch: usize, beta: &[f32], gamma: &[f32], out: &mut [f32]) {
        let n = CELLS * ch;
        let m: f32 = x[..n].iter().sum::<f32>() / n as f32;
        for p in 0..CELLS {
            let b = p * ch;
            for c in 0..ch {
                let d = x[b + c] - m;
                out[b + c] = gamma[c] * d / (d * d + 1e-3).sqrt() + beta[c];
            }
        }
    }

    /// out[q*128+co] = sum_ci t[q*128+ci] * W[ci*128+co] (dense, shared across the 81 cells).
    fn matmul81(t: &[f32], w: &[f32], out: &mut [f32]) {
        out.iter_mut().for_each(|v| *v = 0.0);
        for q in 0..CELLS {
            let t_base = q * C;
            let o_base = q * C;
            for ci in 0..C {
                let tv = t[t_base + ci];
                if tv == 0.0 {
                    continue;
                }
                let w_row = ci * C;
                for co in 0..C {
                    out[o_base + co] += tv * w[w_row + co];
                }
            }
        }
    }

    /// Adds multi-head attention to `h` using the original Q/K/softmax path.
    fn add_attention_generic(h: &mut [f32], q: &[f32], k: &[f32], v: &[f32]) {
        let inv_sqrt = 1.0f32 / (32.0f32).sqrt();
        let mut s = [0.0f32; CELLS];
        for hd in 0..4 {
            let o = hd * 32;
            for qi in 0..CELLS {
                let q_base = qi * C + o;
                let mut mx = f32::NEG_INFINITY;
                for ki in 0..CELLS {
                    let k_base = ki * C + o;
                    let mut dot = 0.0f32;
                    for d in 0..32 {
                        dot += q[q_base + d] * k[k_base + d];
                    }
                    dot *= inv_sqrt;
                    s[ki] = dot;
                    if dot > mx {
                        mx = dot;
                    }
                }
                let mut z = 0.0f32;
                for ki in 0..CELLS {
                    let e = (s[ki] - mx).exp();
                    s[ki] = e;
                    z += e;
                }
                let o_base = qi * C + o;
                for d in 0..32 {
                    let mut acc = 0.0f32;
                    for ki in 0..CELLS {
                        acc += s[ki] * v[ki * C + o + d];
                    }
                    h[o_base + d] += acc / z;
                }
            }
        }
    }

    /// Adds uniform attention over V. This is equivalent to the generic path
    /// when Q and K are zero, while avoiding Q/K matmuls, score calculation,
    /// softmax, and per-query weighted sums.
    fn add_uniform_attention(h: &mut [f32], v: &[f32]) {
        let mut mean = [0.0f32; C];
        for cell in 0..CELLS {
            let base = cell * C;
            for channel in 0..C {
                mean[channel] += v[base + channel];
            }
        }
        for value in &mut mean {
            *value /= CELLS as f32;
        }
        for cell in 0..CELLS {
            let base = cell * C;
            for channel in 0..C {
                h[base + channel] += mean[channel];
            }
        }
    }

    fn build_pos_arr() -> Vec<f32> {
        let mut pos = vec![0.0f32; CELLS * C];
        for x in 0..9i32 {
            for y in 0..9i32 {
                let base = ((x * 9 + y) as usize) * C;
                pos[base] = (x - 4) as f32 / 4.0;
                pos[base + 1] = (y - 4) as f32 / 4.0;
                pos[base + 2] = (x % 3 - 1) as f32;
                pos[base + 3] = (y % 3 - 1) as f32;
                pos[base + 4] = (x / 3 - 1) as f32;
                pos[base + 5] = (y / 3 - 1) as f32;
            }
        }
        pos
    }

    /// feat: [1215] laid out feat[(x*9+y)*15+c]. Returns the 137-way policy
    /// softmax only (no value head — scoped out for this experiment).
    pub fn forward_policy(&self, feat: &[f32; 1215]) -> [f32; 137] {
        let pos_arr = Self::build_pos_arr();
        let mut h = vec![0.0f32; CELLS * C];
        let mut f = vec![0.0f32; CELLS * C];
        let mut t = vec![0.0f32; CELLS * C];

        // layer 0
        Self::conv(feat, 15, C, &self.w_conv0, 3, &mut f);
        Self::norm(&f, C, &self.beta[0], &self.gamma[0], &mut h);
        for k in 0..h.len() {
            h[k] = h[k].max(0.0) + pos_arr[k];
        }

        // trunk 1..17
        for i in 1..TRUNK_LAYERS {
            if !ATTN_LAYERS.contains(&i) {
                let k = if i <= 5 { 3 } else { 1 };
                let wc = &self.w_conv[&i];
                Self::conv(&h, C, C, wc, k, &mut t);
                Self::norm(&t, C, &self.beta[i], &self.gamma[i], &mut f);
                for k in 0..f.len() {
                    let v = f[k];
                    if v > 0.0 {
                        h[k] += v;
                    }
                }
            } else {
                Self::norm(&h, C, &self.beta[i], &self.gamma[i], &mut t);
                let mut v = vec![0.0f32; CELLS * C];
                Self::matmul81(&t, &self.wv[&i], &mut v);
                if self.uniform_attention[i] {
                    Self::add_uniform_attention(&mut h, &v);
                } else {
                    let mut q = vec![0.0f32; CELLS * C];
                    let mut kk = vec![0.0f32; CELLS * C];
                    Self::matmul81(&t, &self.wq[&i], &mut q);
                    Self::matmul81(&t, &self.wk[&i], &mut kk);
                    Self::add_attention_generic(&mut h, &q, &kk, &v);
                }
            }
        }

        // policy head: conv 128->32 (3x3, bias, ReLU) -> flatten 2592 -> FC -> softmax(137)
        let mut ph = vec![0.0f32; CELLS * 32];
        Self::conv(&h, C, 32, &self.w_p_head_conv, 3, &mut ph);
        for p in 0..CELLS {
            for c in 0..32 {
                let v = ph[p * 32 + c] + self.b_p_head_conv[c];
                ph[p * 32 + c] = v.max(0.0);
            }
        }
        let mut logits = [0.0f32; 137];
        logits.copy_from_slice(&self.b2);
        for k in 0..(CELLS * 32) {
            let v = ph[k];
            if v == 0.0 {
                continue;
            }
            let w_row = k * 137;
            for a in 0..137 {
                logits[a] += v * self.w2[w_row + a];
            }
        }
        let mx = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut p = [0.0f32; 137];
        let mut z = 0.0f32;
        for a in 0..137 {
            let e = (logits[a] - mx).exp();
            p[a] = e;
            z += e;
        }
        for a in 0..137 {
            p[a] /= z;
        }
        p
    }
}

/// Board-geometric wall legality, ignoring walls-in-hand count (feature planes
/// are board-geometric, per ka_encoder.js's `wallPlacableUngated`). Mutates
/// wall bits transiently to run the path check, restoring them before return.
fn wall_placable_ungated(g: &mut GameState, wall_type: usize, slot: usize) -> bool {
    if !g.wall_fits(wall_type, slot) {
        return false;
    }
    if !g.wall_needs_path_check(wall_type, slot) {
        return true;
    }
    g.set_wall_bits(wall_type, slot, true);
    let ok = g.has_path(0) && g.has_path(1);
    g.set_wall_bits(wall_type, slot, false);
    ok
}

/// 1:1 port of ka_encoder.js's `encode()`. Layout: feat[(x*9+y)*15+c], x=col,
/// y=row (same R*9+C frame GameState already uses — no coordinate flip
/// needed, unlike the wall-move-id translation in mod.rs). Root-only use
/// intended: calls `compute_dist` twice and probes all 128 wall slots, not
/// cheap enough for per-node use (same cost class as the net itself).
pub fn encode_ka_features(g: &mut GameState) -> [f32; 1215] {
    let mut out = [0.0f32; 1215];
    for p in 0..2 {
        let cell = g.pawn[p];
        let r = cell / 9;
        let c = cell % 9;
        out[(c * 9 + r) * 15 + p] = 1.0;
    }
    let w0 = g.wl[0] as f32 / 10.0;
    let w1 = g.wl[1] as f32 / 10.0;
    let tm = (g.turn % 2) as f32;
    let mut dist0 = [0u8; 81];
    let mut dist1 = [0u8; 81];
    g.compute_dist(0, &mut dist0);
    g.compute_dist(1, &mut dist1);
    let d0 = dist0[g.pawn[0]] as f32 / 20.0;
    let d1 = dist1[g.pawn[1]] as f32 / 20.0;
    // (channel, our-dir) pairs — our dir 0=N,1=S,2=W,3=E (game.rs DELTA order)
    // matches ace.html's KA_CH_DIR = [[5,0],[6,3],[7,1],[8,2]].
    const KA_CH_DIR: [(usize, usize); 4] = [(5, 0), (6, 3), (7, 1), (8, 2)];
    for x in 0..9usize {
        for y in 0..9usize {
            let base = (x * 9 + y) * 15;
            out[base + 2] = w0;
            out[base + 3] = w1;
            out[base + 4] = tm;
            out[base + 9] = d0;
            out[base + 10] = d1;
            let our_cell = y * 9 + x;
            for &(ch, dir) in KA_CH_DIR.iter() {
                out[base + ch] = if g.can_step(our_cell, dir) { 1.0 } else { 0.0 };
            }
        }
    }
    for slot in 0..64usize {
        let wr = slot / 8;
        let wc = slot % 8;
        let wb = (wc * 9 + wr) * 15;
        if g.hw[slot] != 0 {
            out[wb + 11] = 1.0;
        }
        if g.vw[slot] != 0 {
            out[wb + 12] = 1.0;
        }
        if wall_placable_ungated(g, 0, slot) {
            out[wb + 13] = 1.0;
        }
        if wall_placable_ungated(g, 1, slot) {
            out[wb + 14] = 1.0;
        }
    }
    out
}

fn enc_sign(v: i32) -> i32 {
    if v == -1 {
        2
    } else {
        v
    }
}

/// Titanium move id -> Ka's 137-action index (1:1 port of
/// ka_encoder.js's `ourMoveToKaId`; Titanium's move-id convention is the same
/// "our" scheme ace.html's ka_ab/ka_engine already use, so no adapter needed
/// beyond this arithmetic). `from_cell` is the mover's pawn cell BEFORE the move.
pub fn move_id_to_ka_action(m: i16, from_cell: usize) -> usize {
    if m >= 100 {
        let is_h = m < 200;
        let slot = (m - if is_h { 100 } else { 200 }) as i32;
        let wr = slot / 8;
        let wc = slot % 8;
        (if is_h { 0 } else { 64 } + wc * 8 + wr) as usize
    } else {
        let fx = (from_cell % 9) as i32;
        let fy = (from_cell / 9) as i32;
        let tx = (m as i32) % 9;
        let ty = (m as i32) / 9;
        let dx = tx - fx;
        let dy = ty - fy;
        let sx = if dx == 0 {
            0
        } else if dx > 0 {
            1
        } else {
            -1
        };
        let sy = if dy == 0 {
            0
        } else if dy > 0 {
            1
        } else {
            -1
        };
        (128 + enc_sign(sx) * 3 + enc_sign(sy)) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn uniform_attention_matches_generic_zero_qk_attention() {
        let q = vec![0.0f32; CELLS * C];
        let k = vec![0.0f32; CELLS * C];
        let mut v = vec![0.0f32; CELLS * C];
        let mut generic_h = vec![0.25f32; CELLS * C];
        let mut uniform_h = generic_h.clone();
        for cell in 0..CELLS {
            for channel in 0..C {
                v[cell * C + channel] = ((cell * 17 + channel * 3) % 19) as f32 - 9.0;
            }
        }

        KaPolicyNet::add_attention_generic(&mut generic_h, &q, &k, &v);
        KaPolicyNet::add_uniform_attention(&mut uniform_h, &v);

        assert_eq!(generic_h, uniform_h);
    }

    #[test]
    fn uniform_attention_pruning_respects_threshold() {
        let mut q = vec![0.0f32; C * C];
        let k = vec![0.0f32; C * C];
        q[0] = UNIFORM_ATTENTION_QK_MAX_ABS;
        assert!(has_effectively_uniform_attention(&q, &k));

        q[0] = UNIFORM_ATTENTION_QK_MAX_ABS * 1.001;
        assert!(!has_effectively_uniform_attention(&q, &k));
    }

    /// Loads the fixed-layout fixture written by
    /// reference/ka_weights_export/ (1215 f32 feature + 137 f32 policy +
    /// 1 f32 value per case). Both this file and the weight export are
    /// gitignored (private, unclear-license third-party checkpoint), so this
    /// test is a no-op skip — not a failure — on any machine without them.
    fn read_cases(path: &Path) -> Vec<([f32; 1215], [f32; 137], f32)> {
        let mut f = std::fs::File::open(path).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let case_bytes = (1215 + 137 + 1) * 4;
        assert_eq!(buf.len() % case_bytes, 0, "unexpected fixture size");
        let mut cases = Vec::new();
        for chunk in buf.chunks_exact(case_bytes) {
            let mut floats = chunk
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
            let mut feat = [0.0f32; 1215];
            for v in feat.iter_mut() {
                *v = floats.next().unwrap();
            }
            let mut p = [0.0f32; 137];
            for v in p.iter_mut() {
                *v = floats.next().unwrap();
            }
            let value = floats.next().unwrap();
            cases.push((feat, p, value));
        }
        cases
    }

    #[test]
    fn matches_js_reference_forward() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let weights_path =
            repo_root.join("reference/ka_weights_export/ka_policy_trunk_selfdesc.bin");
        let cases_path = repo_root.join("reference/ka_weights_export/ref_cases.bin");
        if !weights_path.exists() || !cases_path.exists() {
            eprintln!(
                "skip: ka_policy reference fixtures not present at {:?} (private/gitignored, generated locally only)",
                weights_path
            );
            return;
        }
        let net = KaPolicyNet::load(&weights_path)
            .expect("read weights file")
            .expect("weights file exists per check above");
        let cases = read_cases(&cases_path);
        assert!(!cases.is_empty());
        for (i, (feat, expected_p, _value)) in cases.iter().enumerate() {
            let got_p = net.forward_policy(feat);
            let mut max_abs_diff = 0.0f32;
            let mut argmax_got = 0usize;
            let mut argmax_exp = 0usize;
            for a in 0..137 {
                let d = (got_p[a] - expected_p[a]).abs();
                if d > max_abs_diff {
                    max_abs_diff = d;
                }
                if got_p[a] > got_p[argmax_got] {
                    argmax_got = a;
                }
                if expected_p[a] > expected_p[argmax_exp] {
                    argmax_exp = a;
                }
            }
            assert_eq!(
                argmax_got, argmax_exp,
                "case {i}: argmax mismatch (got {argmax_got}, expected {argmax_exp})"
            );
            assert!(
                max_abs_diff < 1e-3,
                "case {i}: max abs diff {max_abs_diff} exceeds tolerance"
            );
        }
    }

    #[test]
    fn forward_pass_latency_bench() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let weights_path =
            repo_root.join("reference/ka_weights_export/ka_policy_trunk_selfdesc.bin");
        if !weights_path.exists() {
            eprintln!("skip: ka_policy weights not present locally");
            return;
        }
        let net = KaPolicyNet::load(&weights_path)
            .expect("read weights file")
            .expect("weights file exists per check above");
        let feat = [0.1f32; 1215];
        // warmup
        for _ in 0..3 {
            std::hint::black_box(net.forward_policy(&feat));
        }
        let n = 50;
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            std::hint::black_box(net.forward_policy(&feat));
        }
        let elapsed = t0.elapsed();
        let per_call = elapsed / n;
        eprintln!(
            "ka_policy forward_policy: {n} calls in {:?} -> {:?}/call",
            elapsed, per_call
        );
    }
}
