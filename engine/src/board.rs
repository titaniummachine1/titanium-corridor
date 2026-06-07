//! Quoridor board — internal 0..8 rows/cols, wall bitboards 64-bit each.

pub type Row = u8;
pub type Column = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Player {
    One = 0,
    Two = 1,
}

impl Player {
    pub fn opposite(self) -> Self {
        match self {
            Player::One => Player::Two,
            Player::Two => Player::One,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WallOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Move {
    Pawn { row: Row, col: Column },
    Wall {
        row: Row,
        col: Column,
        orientation: WallOrientation,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Board {
    pub pawns: [(Row, Column); 2],
    pub walls_remaining: [u8; 2],
    pub horizontal_walls: u64,
    pub vertical_walls: u64,
    pub side_to_move: Player,
    pub move_number: u32,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    pub fn new() -> Self {
        Self {
            pawns: [(0, 4), (8, 4)],
            walls_remaining: [10, 10],
            horizontal_walls: 0,
            vertical_walls: 0,
            side_to_move: Player::One,
            move_number: 1,
        }
    }

    #[inline]
    pub fn pawn(&self, player: Player) -> (Row, Column) {
        self.pawns[player as usize]
    }

    #[inline]
    pub fn side(&self) -> Player {
        self.side_to_move
    }

    pub fn column_char(col: Column) -> char {
        (b'a' + col) as char
    }

    pub fn format_square(row: Row, col: Column) -> String {
        format!("{}{}", Self::column_char(col), row + 1)
    }

    pub fn is_terminal(&self) -> Option<Player> {
        if self.pawns[0].0 == 8 {
            return Some(Player::One);
        }
        if self.pawns[1].0 == 0 {
            return Some(Player::Two);
        }
        None
    }

    pub fn apply_move(&mut self, mv: Move) {
        let side = self.side_to_move as usize;
        match mv {
            Move::Pawn { row, col } => {
                self.pawns[side] = (row, col);
            }
            Move::Wall {
                row,
                col,
                orientation,
            } => {
                crate::grid::set_wall(self, row, col, orientation, true);
                self.walls_remaining[side] -= 1;
            }
        }
        self.side_to_move = self.side_to_move.opposite();
        if self.side_to_move == Player::One {
            self.move_number += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_position_matches_scraped_ui() {
        let board = Board::new();
        assert_eq!(board.pawns[0], (0, 4));
        assert_eq!(board.pawns[1], (8, 4));
        assert_eq!(board.walls_remaining, [10, 10]);
        assert_eq!(board.side_to_move, Player::One);
        assert_eq!(Board::format_square(0, 4), "e1");
        assert_eq!(Board::format_square(8, 4), "e9");
    }
}
