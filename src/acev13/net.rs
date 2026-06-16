//! ACE v10 HalfPW net — weights from `net_weights.bin` (H=32 NET_DATA blob).
//!
//! Philosophy: NN = geometric prior, search = tactical proof. See `field_planes.rs`.
//!
//! Blob layout (little-endian f64):
//!   Wskip[16] B1[32] W2[32] W1C[36864] PO[2592] PX[2592]
//!   goal_inv_p0, goal_inv_p1, pawn_fwd_p0, pawn_fwd_p1,
//!   corridor_delta_p0, corridor_delta_p1, path_cross_p0, path_cross_p1,
//!   choke_p0, choke_p1, contested  (each 81×32 except contested is shared)

use std::sync::OnceLock;

pub const NET_H: usize = 32;
pub const WSKIP_LEN: usize = 16;
const W1C_LEN: usize = 9 * 128 * NET_H;
const PO_LEN: usize = 81 * NET_H;
const PX_LEN: usize = 81 * NET_H;
const FIELD_PLANE_LEN: usize = 81 * NET_H;
const FIELD_PLANE_SETS: usize = 11;

pub const NET_WEIGHT_F64S: usize =
    WSKIP_LEN + NET_H + NET_H + W1C_LEN + PO_LEN + PX_LEN + FIELD_PLANE_LEN * FIELD_PLANE_SETS;

static NET_BYTES: &[u8] = include_bytes!("net_weights.bin");

pub struct Net {
    pub ws: [f64; 16],
    pub b1: [f64; NET_H],
    pub w2: [f64; NET_H],
    pub w1c: Vec<f64>,
    pub po: Vec<f64>,
    pub px: Vec<f64>,
    /// Inverse goal distance field, player 0 (ACE cells).
    pub goal_inv_p0: Vec<f64>,
    /// Inverse goal distance field, player 1.
    pub goal_inv_p1: Vec<f64>,
    /// Steps-from-pawn field, player 0.
    pub pawn_fwd_p0: Vec<f64>,
    /// Steps-from-pawn field, player 1.
    pub pawn_fwd_p1: Vec<f64>,
    /// Corridor delta (multi-path band), player 0.
    pub corridor_delta_p0: Vec<f64>,
    /// Corridor delta, player 1.
    pub corridor_delta_p1: Vec<f64>,
    /// Path-crossing count (route overlap), player 0.
    pub path_cross_p0: Vec<f64>,
    /// Path-crossing count, player 1.
    pub path_cross_p1: Vec<f64>,
    /// Route forcedness (1/(1+forward_continuations)), player 0.
    pub choke_p0: Vec<f64>,
    /// Route forcedness, player 1.
    pub choke_p1: Vec<f64>,
    /// Square on both players' shortest-route families.
    pub contested: Vec<f64>,
}

fn read_f64s(bytes: &[u8], offset: &mut usize, count: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let chunk: [u8; 8] = bytes[*offset..*offset + 8].try_into().unwrap();
        out.push(f64::from_le_bytes(chunk));
        *offset += 8;
    }
    out
}

pub fn net() -> &'static Net {
    static NET: OnceLock<Net> = OnceLock::new();
    NET.get_or_init(|| {
        assert_eq!(
            NET_BYTES.len(),
            NET_WEIGHT_F64S * 8,
            "net_weights.bin size mismatch — run training/extend_field_planes.py"
        );
        let mut offset = 0;
        let ws_v = read_f64s(NET_BYTES, &mut offset, WSKIP_LEN);
        let b1_v = read_f64s(NET_BYTES, &mut offset, NET_H);
        let w2_v = read_f64s(NET_BYTES, &mut offset, NET_H);
        let w1c = read_f64s(NET_BYTES, &mut offset, W1C_LEN);
        let po = read_f64s(NET_BYTES, &mut offset, PO_LEN);
        let px = read_f64s(NET_BYTES, &mut offset, PX_LEN);
        let goal_inv_p0 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let goal_inv_p1 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let pawn_fwd_p0 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let pawn_fwd_p1 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let corridor_delta_p0 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let corridor_delta_p1 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let path_cross_p0 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let path_cross_p1 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let choke_p0 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let choke_p1 = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        let contested = read_f64s(NET_BYTES, &mut offset, FIELD_PLANE_LEN);
        Net {
            ws: ws_v.try_into().unwrap(),
            b1: b1_v.try_into().unwrap(),
            w2: w2_v.try_into().unwrap(),
            w1c,
            po,
            px,
            goal_inv_p0,
            goal_inv_p1,
            pawn_fwd_p0,
            pawn_fwd_p1,
            corridor_delta_p0,
            corridor_delta_p1,
            path_cross_p0,
            path_cross_p1,
            choke_p0,
            choke_p1,
            contested,
        }
    })
}

// ── Symmetry tables (match the JS NET_MIRC / NET_MIRS / NET_BKT loops) ────────

const fn build_mirc() -> [usize; 81] {
    let mut arr = [0usize; 81];
    let mut i = 0;
    while i < 81 {
        arr[i] = (8 - i / 9) * 9 + i % 9;
        i += 1;
    }
    arr
}

const fn build_mirs() -> [usize; 64] {
    let mut arr = [0usize; 64];
    let mut i = 0;
    while i < 64 {
        arr[i] = (7 - i / 8) * 8 + i % 8;
        i += 1;
    }
    arr
}

const fn build_bkt() -> [usize; 81] {
    let mut arr = [0usize; 81];
    let mut i = 0;
    while i < 81 {
        arr[i] = (i / 9 / 3) * 3 + (i % 9) / 3;
        i += 1;
    }
    arr
}

pub static NET_MIRC: [usize; 81] = build_mirc();
pub static NET_MIRS: [usize; 64] = build_mirs();
pub static NET_BKT: [usize; 81] = build_bkt();
