//! Checkpoint 01 CLI — path distance smoke test.

use titanium::{Board, Player, shortest_distance};

fn main() {
    let board = Board::new();
    println!("Titanium Engine 0.1.0 — checkpoint 01 (BFS)");
    println!(
        "P1 {} → goal dist {:?}",
        Board::format_square(board.pawns[0].0, board.pawns[0].1),
        shortest_distance(&board, Player::One),
    );
    println!(
        "P2 {} → goal dist {:?}",
        Board::format_square(board.pawns[1].0, board.pawns[1].1),
        shortest_distance(&board, Player::Two),
    );
}
