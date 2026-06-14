//! Transposition table — perft node cache now, αβ search later.
//!
//! Stockfish-style **clustered buckets** (4 slots per index) to cut collisions.

const TT_CLUSTER: usize = 4;
/// Cluster size in bytes: 4 entries × 24 B = 96 B.
const TT_CLUSTER_BYTES: usize = TT_CLUSTER * 24;

/// Size/performance table (native, perft, `tt_speedup` bench):
///   | bits | RAM    | perft(3) | perft(4) | perft(5) |  fits in        |
///   |   9  |  48 KB |  105 ms  |  1.05 s  |    —     |  L1/core (64K) | ← start
///   |  11  | 192 KB |  103 ms  |  0.83 s  |    —     |  L2/core (256K)| ← L1→L2 jump
///   |  16  |   6 MB |  111 ms  |  0.44 s  |    —     |  L3 (8 MB)     | ← L2→L3 jump
///   |  18  |  24 MB |  119 ms  |  0.37 s  | 20.7 s   |                | ← d4 optimal
///   |  22  | 384 MB |  119 ms  |  0.63 s  | 12.6 s   |                | ← d5 optimal
///   |  24  | 1.5 GB |    —     |  0.76 s  | 13.4 s   |                |
///
/// Working-set knee per depth: d3≈11, d4≈18, d5≈22 (~7 bits per ply).
/// Larger tables regress at shallow depth (page-fault/TLB pressure).
///
/// **Adaptive mode (default) — three-phase cache-tier grow:**
///   Phase 1 (L1) — start at `DEFAULT_START_BITS` (9, 48 KB inside L1/core).
///                  d1/d2 never fill this; they pay zero page-fault cost.
///   Phase 2 (L2) — on L1 overflow, jump to `DEFAULT_L2_BITS` (11, 192 KB).
///                  Rehashes only the 48 KB L1 table — trivially fast.
///                  d3's working set fits here (optimal for search-depth d3).
///   Phase 3 (L3) — on L2 overflow, jump to `DEFAULT_L3_BITS` (16, 6 MB).
///                  Rehashes only the 192 KB L2 table — still cheap.
///   Phase 4 (+1) — +1 bit per overflow past L3 (careful steps for d4/d5).
///
/// Each tier jump rehashes only the CURRENT (small) table, not a ladder of
/// intermediate sizes. Cleared TT retains its grown size across searches so
/// re-warm calls within a session never pay the grow cost again.
///
/// Override env vars (all accept 8..=27):
///   `TT_BITS=N`      — pin static size, disable growth (benchmarking)
///   `TT_START_BITS`  — L1-phase start (default 9)
///   `TT_L2_BITS`     — L1→L2 jump target (default 11)
///   `TT_L3_BITS`     — L2→L3 jump target (default 16)
///   `TT_MAX_BITS`    — growth ceiling (default 25, ~3.2 GB)

// Hardware calibration (i7-4900MQ, 4 cores / 8 threads):
//   L1 data/core = 32 KB  (total L1 = 256 KB / 4 = 64 KB/core incl. instr.)
//   L2/core      = 256 KB (total L2 = 1 MB / 4 cores)
//   L3 shared    = 8 MB
//   96 B/cluster: 2^9=512  → 49 KB fits L1/core; 2^11=2048 → 192 KB fits L2/core;
//                 2^16=64K → 6 MB fits L3.
const DEFAULT_START_BITS: usize = 9;  // 48 KB — L1/core
const DEFAULT_L2_BITS: usize = 11;   // 192 KB — L2/core
const DEFAULT_L3_BITS: usize = 16;   // 6 MB — L3
const DEFAULT_MAX_BITS: usize = 25;  // 3.2 GB ceiling

// NOTE: 16-byte packed layout tried at perft(4) and (5) — no speedup. Engine
// is compute-bound on TT-miss nodes, not memory-bound. See `benches/tt_speedup.rs`.
//
// COLLISION SAFETY: 64-bit `key` alone can't prove board identity. `verify` is an
// independent 32-bit hash (`Board::tt_verify`); false hit needs BOTH (~2^-96/pair).
// FREE at current struct size: {key:8, nodes:8, verify:4, depth:1} = 21 B → 24 B.
//
// EVICTION: depth-only (shallowest entry in a full cluster). walls_total-primary
// policy MEASURED and REJECTED: regressed d5 ~10% (shallow nodes carry the most
// plies below — evicting them first tanks hit rate).
#[derive(Clone, Copy, Default)]
struct Entry {
    key: u64,
    nodes: u64,
    verify: u32,
    depth: u8,
}

#[derive(Clone, Copy)]
struct Cluster {
    entries: [Entry; TT_CLUSTER],
}

impl Default for Cluster {
    fn default() -> Self {
        Self { entries: [Entry::default(); TT_CLUSTER] }
    }
}

pub struct TranspositionTable {
    clusters: Vec<Cluster>,
    mask: usize,
    bits: usize,
    /// Non-empty slots consumed (grow trigger).
    filled: usize,
    /// L1→L2 jump target bits.
    l2_bits: usize,
    /// L2→L3 jump target bits.
    l3_bits: usize,
    max_bits: usize,
    /// False when `TT_BITS` was pinned (static, no growth).
    adaptive: bool,
}

impl Default for TranspositionTable {
    fn default() -> Self { Self::new() }
}

fn env_bits(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&b| (8..=27).contains(&b))
}

impl TranspositionTable {
    pub fn new() -> Self {
        // `TT_BITS` pins a static size and disables adaptive growth entirely.
        if let Some(bits) = env_bits("TT_BITS") {
            return Self::make(bits, bits, bits, bits, false);
        }
        let start = env_bits("TT_START_BITS").unwrap_or(DEFAULT_START_BITS);
        let l2    = env_bits("TT_L2_BITS").unwrap_or(DEFAULT_L2_BITS).max(start);
        let l3    = env_bits("TT_L3_BITS").unwrap_or(DEFAULT_L3_BITS).max(l2);
        let max   = env_bits("TT_MAX_BITS").unwrap_or(DEFAULT_MAX_BITS).max(l3);
        Self::make(start, l2, l3, max, true)
    }

    fn make(bits: usize, l2_bits: usize, l3_bits: usize, max_bits: usize, adaptive: bool) -> Self {
        let size = 1usize << bits;
        Self {
            clusters: vec![Cluster::default(); size],
            mask: size - 1,
            bits,
            filled: 0,
            l2_bits,
            l3_bits,
            max_bits,
            adaptive,
        }
    }

    pub fn clear(&mut self) {
        self.clusters.fill(Cluster::default());
        self.filled = 0;
        // Size is NOT reset — subsequent searches reuse the grown table and
        // skip the tier-jump cost entirely.
    }

    /// Current allocation in bytes (diagnostics / logging).
    pub fn size_bytes(&self) -> usize {
        self.clusters.len() * TT_CLUSTER_BYTES
    }

    #[inline]
    fn should_grow(&self) -> bool {
        self.adaptive
            && self.bits < self.max_bits
            && self.filled * 2 >= self.clusters.len() * TT_CLUSTER
    }

    #[inline]
    pub fn probe(&self, key: u64, verify: u32, depth: u8) -> Option<u64> {
        let cluster = &self.clusters[(key as usize) & self.mask];
        for entry in &cluster.entries {
            if entry.key == key && entry.verify == verify && entry.depth == depth {
                return Some(entry.nodes);
            }
        }
        None
    }

    #[inline]
    pub fn store(&mut self, key: u64, verify: u32, depth: u8, nodes: u64) {
        if Self::insert_into(&mut self.clusters, self.mask, key, verify, depth, nodes) {
            self.filled += 1;
            if self.should_grow() {
                self.grow();
            }
        }
    }

    /// Insert into `(clusters, mask)`. Returns `true` iff an empty slot was
    /// consumed (for occupancy tracking). Updates and evictions return `false`.
    #[inline]
    fn insert_into(
        clusters: &mut [Cluster],
        mask: usize,
        key: u64,
        verify: u32,
        depth: u8,
        nodes: u64,
    ) -> bool {
        let cluster = &mut clusters[(key as usize) & mask];
        let mut replace = 0usize;
        let mut shallowest = u8::MAX;

        for (i, entry) in cluster.entries.iter().enumerate() {
            if entry.key == key && entry.verify == verify {
                if entry.depth <= depth {
                    cluster.entries[i] = Entry { key, nodes, verify, depth };
                }
                return false;
            }
            if entry.key == 0 {
                cluster.entries[i] = Entry { key, nodes, verify, depth };
                return true;
            }
            if entry.depth < shallowest {
                shallowest = entry.depth;
                replace = i;
            }
        }
        cluster.entries[replace] = Entry { key, nodes, verify, depth };
        false
    }

    /// Grow the table using cache-tier jumps, then fine-grained +1 past L3.
    ///
    /// On L1 overflow → jump to L2 (rehash ~48 KB).
    /// On L2 overflow → jump to L3 (rehash ~192 KB).
    /// On L3 overflow → +1 bit (rehash grows gradually for d4/d5).
    ///
    /// Each jump rehashes only the current (small) table, never a ladder of
    /// intermediate sizes.
    fn grow(&mut self) {
        let new_bits = if self.bits < self.l2_bits {
            self.l2_bits                  // L1 → L2 in one shot
        } else if self.bits < self.l3_bits {
            self.l3_bits                  // L2 → L3 in one shot
        } else {
            self.bits + 1                 // fine-grained past L3
        }
        .min(self.max_bits);

        let new_size = 1usize << new_bits;
        let new_mask = new_size - 1;
        let mut next = vec![Cluster::default(); new_size];
        for cluster in &self.clusters {
            for e in &cluster.entries {
                if e.key != 0 {
                    Self::insert_into(&mut next, new_mask, e.key, e.verify, e.depth, e.nodes);
                }
            }
        }
        self.clusters = next;
        self.mask = new_mask;
        self.bits = new_bits;
    }
}
