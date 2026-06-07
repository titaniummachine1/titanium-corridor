//! One-ply greedy search — maximize (opp_dist - our_dist) after each legal move.

use crate::board::{Board, Move, Player};
use crate::moves::{generate_legal_moves_slice, MAX_LEGAL_MOVES};
use crate::path::BfsScratch;
use crate::perft::format_move;

const DIST_PENALTY: u8 = 255;

fn eval_after_move(board: &Board, player: Player, scratch: &mut BfsScratch) -> i32 {
    let us = player;
    let opp = player.opposite();
    let our_dist = i32::from(scratch.shortest_distance(board, us).unwrap_or(DIST_PENALTY));
    let opp_dist = i32::from(scratch.shortest_distance(board, opp).unwrap_or(DIST_PENALTY));
    opp_dist - our_dist
}

fn move_tie_break(mv: Move) -> i32 {
    match mv {
        Move::Pawn { .. } => 1,
        Move::Wall { .. } => 0,
    }
}

/// Best move for `board` by one-ply path-distance heuristic.
pub fn choose_greedy_move(board: &mut Board, scratch: &mut BfsScratch) -> Option<Move> {
    let player = board.side();
    let mut buf = [Move::Pawn { row: 0, col: 0 }; MAX_LEGAL_MOVES];
    let n = generate_legal_moves_slice(board, &mut buf, scratch);
    if n == 0 {
        return None;
    }

    let mut best_mv = buf[0];
    let mut best_score = i32::MIN;
    let mut best_tie = move_tie_break(best_mv);

    for mv in buf.iter().take(n) {
        let undo = board.make_move(*mv);
        let score = eval_after_move(board, player, scratch);
        board.unmake_move(undo);

        let tie = move_tie_break(*mv);
        if score > best_score || (score == best_score && tie > best_tie) {
            best_score = score;
            best_mv = *mv;
            best_tie = tie;
        }
    }

    Some(best_mv)
}

/// CLI helper — algebraic best move for a position given as prior plies.
pub fn genmove_algebraic(board: &mut Board, scratch: &mut BfsScratch) -> Option<String> {
    choose_greedy_move(board, scratch).map(format_move)
}
