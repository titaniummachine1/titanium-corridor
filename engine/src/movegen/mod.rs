//! Legal move generation (no pruning — see `cat::prune`).

pub mod legal;
pub mod pawn_bits;

pub use legal::{
    generate_legal_moves, generate_legal_moves_into, generate_legal_moves_slice, MAX_LEGAL_MOVES,
};
pub use pawn_bits::{
    generate_pawn_moves_bitboard_slice, generate_pawn_moves_bitboard_with_masks,
};
