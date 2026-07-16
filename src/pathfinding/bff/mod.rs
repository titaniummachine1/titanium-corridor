//! Bitwise flood-fill primitives (centered 11-wide u128 layout).
//! Shared Binary Flood Fill (BFF) building blocks for BFS fields and wall trials.

pub mod wall;

use crate::core::board::Player;
use crate::pathfinding::masks::DirMasks;
#[cfg(test)]
use crate::util::grid::pack_flood_mask;
use crate::util::grid::{
    flood_bit_sq, goal_row, square_index, FLOOD_PLAYABLE, FLOOD_SQ_BY_BIT, FLOOD_STRIDE,
};

#[inline]
pub fn goal_square_mask(player: Player) -> u128 {
    let grow = goal_row(player);
    let mut mask = 0u128;
    for c in 0..9u8 {
        mask |= flood_bit_sq(square_index(grow, c));
    }
    mask
}

/// Expand flood frontier in centered 11-wide layout (side buffers absorb E/W shifts).
#[inline]
pub fn expand_frontier(frontier: u128, masks: DirMasks) -> u128 {
    let north = (frontier & masks.north) >> FLOOD_STRIDE;
    let south = (frontier & masks.south) << FLOOD_STRIDE;
    let east = (frontier & masks.east) << 1;
    let west = (frontier & masks.west) >> 1;
    north | south | east | west
}

/// Exact Lee-wave distance field from an arbitrary bitset seed.
///
/// Every iteration expands one bit-parallel frontier. `out[sq]` is the first
/// wave that reaches `sq` (`u8::MAX` when unreachable), so all dense distance
/// consumers share the same BFF implementation instead of maintaining private
/// scalar queue BFS copies.
pub fn flood_distance_field(seed: u128, masks: DirMasks, out: &mut [u8; 81]) -> u128 {
    out.fill(u8::MAX);
    let mut reached = seed & FLOOD_PLAYABLE;
    let mut frontier = reached;
    let mut depth = 0u8;

    while frontier != 0 {
        let mut bits = frontier;
        while bits != 0 {
            let bit_index = bits.trailing_zeros();
            bits &= bits - 1;
            let sq = FLOOD_SQ_BY_BIT[bit_index as usize];
            debug_assert_ne!(sq, u8::MAX);
            if sq != u8::MAX {
                out[sq as usize] = depth;
            }
        }

        frontier = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        reached |= frontier;
        depth = depth.saturating_add(1);
    }
    reached
}

#[inline]
pub fn flood_fill_flood_bits(start_sq: u8, masks: DirMasks) -> u128 {
    let mut reached = flood_bit_sq(start_sq);
    let mut frontier = reached;
    while frontier != 0 {
        frontier = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        reached |= frontier;
    }
    reached
}

#[inline]
#[cfg(test)]
pub fn flood_fill(start_sq: u8, masks: DirMasks) -> u128 {
    pack_flood_mask(flood_fill_flood_bits(start_sq, masks))
}

/// Flood with cached reachable-mask splice: on first contact with `cache` (visited cells
/// from the other player's flood) annex the whole region — pawn connectivity is
/// undirected — and goal-test the annexed pool immediately, since those cells
/// never re-enter the frontier.
#[inline]
pub fn flood_to_goal_seeded(start_sq: u8, cache: u128, masks: DirMasks, goal_mask: u128) -> bool {
    flood_to_goal_seeded_with_depth(start_sq, cache, masks, goal_mask).0
}

#[inline]
pub fn flood_to_goal_seeded_with_depth(
    start_sq: u8,
    cache: u128,
    masks: DirMasks,
    goal_mask: u128,
) -> (bool, u8) {
    let mut reached = flood_bit_sq(start_sq);
    if reached & goal_mask != 0 {
        return (true, 0);
    }
    let mut frontier = reached;
    let mut pool = cache & !reached;
    let mut depth = 0u8;
    while frontier != 0 {
        if frontier & pool != 0 {
            if pool & goal_mask != 0 {
                return (true, depth);
            }
            reached |= pool;
            frontier |= pool;
            pool = 0;
        }
        frontier = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        if frontier == 0 {
            break;
        }
        depth = depth.saturating_add(1);
        if frontier & goal_mask != 0 {
            return (true, depth);
        }
        reached |= frontier;
    }
    (false, depth)
}

#[inline]
pub fn flood_to_goal(start_sq: u8, masks: DirMasks, goal_mask: u128) -> (bool, u128) {
    let (ok, reached, _) = flood_to_goal_with_depth(start_sq, masks, goal_mask);
    (ok, reached)
}

#[inline]
pub fn flood_to_goal_with_depth(
    start_sq: u8,
    masks: DirMasks,
    goal_mask: u128,
) -> (bool, u128, u8) {
    let mut reached = flood_bit_sq(start_sq);
    if reached & goal_mask != 0 {
        return (true, reached, 0);
    }
    let mut frontier = reached;
    let mut depth = 0u8;
    while frontier != 0 {
        frontier = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        if frontier == 0 {
            break;
        }
        depth = depth.saturating_add(1);
        reached |= frontier;
        if frontier & goal_mask != 0 {
            return (true, reached, depth);
        }
    }
    (false, reached, depth)
}

#[inline]
pub fn flood_component_with_goal_depth(
    start_sq: u8,
    masks: DirMasks,
    goal_mask: u128,
) -> (bool, u128, u8) {
    let mut reached = flood_bit_sq(start_sq);
    let mut frontier = reached;
    let mut depth = 0u8;
    let mut goal_depth = if reached & goal_mask != 0 {
        Some(0)
    } else {
        None
    };
    while frontier != 0 {
        frontier = expand_frontier(frontier, masks) & !reached & FLOOD_PLAYABLE;
        if frontier == 0 {
            break;
        }
        depth = depth.saturating_add(1);
        reached |= frontier;
        if goal_depth.is_none() && frontier & goal_mask != 0 {
            goal_depth = Some(depth);
        }
    }
    match goal_depth {
        Some(d) => (true, reached, d),
        None => (false, reached, depth),
    }
}
