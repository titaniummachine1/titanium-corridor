//! Direction bitmasks for bitwise flood fill on the pawn grid.

use crate::acev13::game::AceGame;
use crate::core::board::Board;
use crate::util::grid::{can_step, flood_bit_sq, square_index};

/// Bit `sq` set iff a pawn on `sq` may step in that direction.
#[derive(Clone, Copy, Default)]
pub struct DirMasks {
    pub north: u128,
    pub south: u128,
    pub east: u128,
    pub west: u128,
}

impl DirMasks {
    pub fn from_board(board: &Board) -> Self {
        let mut m = Self::default();
        for r in 0..=8u8 {
            for c in 0..=8u8 {
                let sq = square_index(r, c);
                let bit = flood_bit_sq(sq);
                if can_step(board, r, c, -1, 0) {
                    m.north |= bit;
                }
                if can_step(board, r, c, 1, 0) {
                    m.south |= bit;
                }
                if can_step(board, r, c, 0, 1) {
                    m.east |= bit;
                }
                if can_step(board, r, c, 0, -1) {
                    m.west |= bit;
                }
            }
        }
        m
    }

    /// Wall topology in ACE cell order (row 0 = top) — no Board rebuild or row remap.
    pub fn from_ace_game(g: &AceGame) -> Self {
        let mut m = Self::default();
        for sq in 0..81usize {
            let bit = flood_bit_sq(sq as u8);
            // ACE `can_step`: 0=N(-9), 1=S(+9), 2=W(-1), 3=E(+1)
            if g.can_step(sq, 0) {
                m.north |= bit;
            }
            if g.can_step(sq, 1) {
                m.south |= bit;
            }
            if g.can_step(sq, 2) {
                m.west |= bit;
            }
            if g.can_step(sq, 3) {
                m.east |= bit;
            }
        }
        m
    }
}
