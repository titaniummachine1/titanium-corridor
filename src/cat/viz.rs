//! CAT v3 visualization snapshot — square heat, wall heat, prune mask for the web UI.

use crate::cat::attention::CorridorAttention;
use crate::cat::constants::{
    BOTTLENECK_BONUS_CM, CAT_COLD_CM, CAT_CORRIDOR_CM, CAT_HOT_CM, DIST_PENALTY,
};
use crate::cat::prune::{
    gap_play_zone_mask, legal_neighbor_denial_heat, wall_completely_skipped, wall_intersects_path,
    wall_shape_attention_bonus,
};
use crate::core::board::{Board, Move, Player};
use crate::movegen::{generate_legal_moves_slice, MAX_LEGAL_MOVES};
use crate::path::BfsScratch;
use crate::util::perft::format_move;

const WALL_SLOT_COUNT: usize = 128;

#[derive(Clone, Copy)]
struct CatWallViz {
    mv: Move,
    direct_heat: u16,
    heat: u16,
    search: bool,
    attention: bool,
    skip: bool,
}

fn wall_path_impact_heat(
    board: &mut Board,
    mv: Move,
    white_dist: u8,
    black_dist: u8,
    route_relevant: bool,
    bfs: &mut BfsScratch,
) -> u16 {
    let Move::Wall { .. } = mv else {
        return 0;
    };
    if !route_relevant {
        return 0;
    }
    let undo = board.make_move(mv);
    let white_after = bfs
        .shortest_distance(board, Player::One)
        .unwrap_or(DIST_PENALTY);
    let black_after = bfs
        .shortest_distance(board, Player::Two)
        .unwrap_or(DIST_PENALTY);
    board.unmake_move(undo);

    let white_gain = u16::from(white_after.saturating_sub(white_dist));
    let black_gain = u16::from(black_after.saturating_sub(black_dist));
    let total = u32::from(white_gain) + u32::from(black_gain);
    if total == 0 {
        return 0;
    }
    let strongest = u32::from(white_gain.max(black_gain));
    let affected_paths = u32::from(white_gain > 0) + u32::from(black_gain > 0);
    let shared_bonus = if affected_paths > 1 { 40 } else { 0 };
    (total * 120 + strongest * 50 + shared_bonus).min(u32::from(u16::MAX)) as u16
}

fn direct_wall_heat(
    board: &mut Board,
    mv: Move,
    search_cat: &CorridorAttention,
    white_cat: &CorridorAttention,
    black_cat: &CorridorAttention,
    white_dist: u8,
    black_dist: u8,
    white_path: &[u8; 81],
    white_path_len: usize,
    black_path: &[u8; 81],
    black_path_len: usize,
    bfs: &mut BfsScratch,
) -> u16 {
    let Move::Wall {
        row,
        col,
        orientation,
    } = mv
    else {
        return 0;
    };
    let white_edge = white_cat.wall_edge_heat(row, col, orientation);
    let black_edge = black_cat.wall_edge_heat(row, col, orientation);
    let route_edge = white_edge
        .max(black_edge)
        .saturating_add(white_edge.min(black_edge).saturating_mul(3) / 4);
    let corridor =
        route_edge.saturating_add(wall_shape_attention_bonus(board, mv, search_cat).max(0) as u16);
    let route_relevant = route_edge > 0
        || wall_intersects_path(mv, white_path, white_path_len)
        || wall_intersects_path(mv, black_path, black_path_len);
    let path = wall_path_impact_heat(board, mv, white_dist, black_dist, route_relevant, bfs);
    corridor.max(path)
}

/// JSON payload for `titanium cat` and `/api/titanium/cat`.
pub fn cat_snapshot_json(board: &mut Board) -> String {
    let mut bfs = BfsScratch::new();
    let cat = bfs.build_corridor_attention(board);
    let white_cat =
        crate::cat::build::build_player_corridor_attention(&mut bfs, board, Player::One);
    let black_cat =
        crate::cat::build::build_player_corridor_attention(&mut bfs, board, Player::Two);

    let mut legal = [Move::Pawn { row: 0, col: 0 }; MAX_LEGAL_MOVES];
    let legal_n = generate_legal_moves_slice(board, &mut legal, &mut bfs);

    let reachable = bfs.both_reachable_mask(board);
    let gap_zone = gap_play_zone_mask(reachable);

    let mut white_path = [0u8; 81];
    let white_path_len =
        crate::cat::prune::get_shortest_path(board, Player::One, &mut bfs, &mut white_path);
    let mut black_path = [0u8; 81];
    let black_path_len =
        crate::cat::prune::get_shortest_path(board, Player::Two, &mut bfs, &mut black_path);

    // Board overlay: per-player max (not summed search heat — that floods mid-game).
    let display_squares = crate::cat::build::build_corridor_display_squares(&mut bfs, board);
    let square_parts: Vec<String> = display_squares.iter().map(|h| h.to_string()).collect();

    let reachable_parts: Vec<&str> = (0u8..81)
        .map(|sq| {
            if reachable & (1u128 << sq) != 0 {
                "1"
            } else {
                "0"
            }
        })
        .collect();

    let white_dist = bfs
        .shortest_distance(board, Player::One)
        .unwrap_or(DIST_PENALTY);
    let black_dist = bfs
        .shortest_distance(board, Player::Two)
        .unwrap_or(DIST_PENALTY);

    let mut walls = Vec::new();
    for i in 0..legal_n {
        let mv = legal[i];
        if !matches!(mv, Move::Wall { .. }) {
            continue;
        };
        let skip = wall_completely_skipped(mv, board, reachable, gap_zone);
        let direct_heat = direct_wall_heat(
            board,
            mv,
            &cat,
            &white_cat,
            &black_cat,
            white_dist,
            black_dist,
            &white_path,
            white_path_len,
            &black_path,
            black_path_len,
            &mut bfs,
        );
        walls.push(CatWallViz {
            mv,
            direct_heat,
            heat: direct_heat,
            search: direct_heat > 0,
            attention: direct_heat > 0,
            skip,
        });
    }

    let mut candidates = [Move::Pawn { row: 0, col: 0 }; WALL_SLOT_COUNT];
    let mut direct_heats = [0i32; WALL_SLOT_COUNT];
    for i in 0..walls.len().min(WALL_SLOT_COUNT) {
        candidates[i] = walls[i].mv;
        direct_heats[i] = i32::from(walls[i].direct_heat);
    }
    for i in 0..walls.len() {
        let counter_heat = legal_neighbor_denial_heat(
            walls[i].mv,
            &candidates,
            &direct_heats,
            walls.len().min(WALL_SLOT_COUNT),
        )
        .max(0) as u16;
        if counter_heat > walls[i].heat {
            walls[i].heat = counter_heat;
            walls[i].attention = true;
            walls[i].search = true;
        }
    }

    let mut wall_parts = Vec::new();
    for wall in walls {
        let alg = format_move(wall.mv);
        wall_parts.push(format!(
            "{{\"alg\":\"{}\",\"heat\":{},\"directHeat\":{},\"search\":{},\"attention\":{},\"skip\":{}}}",
            alg, wall.heat, wall.direct_heat, wall.search, wall.attention, wall.skip
        ));
    }

    let skipped_squares = reachable_parts.iter().filter(|&&b| b == "0").count();

    format!(
        "{{\"squares\":[{}],\"reachable\":[{}],\"walls\":[{}],\"whiteDist\":{},\"blackDist\":{},\"skippedSquares\":{},\"hotCm\":{},\"coldCm\":{},\"maxCm\":{}}}",
        square_parts.join(","),
        reachable_parts.join(","),
        wall_parts.join(","),
        white_dist,
        black_dist,
        skipped_squares,
        CAT_HOT_CM,
        CAT_COLD_CM,
        // Display squares are per-player max (not the summed search table), so the
        // color ceiling is one player's full corridor + bottleneck bonus.
        (CAT_CORRIDOR_CM + BOTTLENECK_BONUS_CM) * 2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::grid::square_index;

    #[test]
    fn snapshot_uses_sparse_corridor_not_full_board_flood() {
        let mut board = Board::new();
        board.apply_algebraic("e2");
        board.apply_algebraic("e8");
        board.apply_algebraic("e3");
        let json = cat_snapshot_json(&mut board);
        let values = parse_snapshot_squares(&json);
        let warm = values.iter().filter(|&&v| v >= CAT_COLD_CM).count();
        assert!(
            warm < 45,
            "corridor CAT should not flood the board, got {warm} warm squares"
        );
        let e3 = values[square_index(2, 4) as usize];
        let a1 = values[square_index(0, 0) as usize];
        assert!(e3 > a1, "pawn corridor hotter than far corner");
        assert!(a1 < CAT_COLD_CM, "far corner stays cold fringe");
    }

    #[test]
    fn snapshot_midgame_corridor_stays_focused() {
        let moves = [
            "e2", "e8", "e3", "e7", "d7v", "e4", "d8v", "f3", "e5", "e6", "b5v",
        ];
        let mut board = Board::new();
        for mv in moves {
            board.apply_algebraic(mv);
        }
        let json = cat_snapshot_json(&mut board);
        let values = parse_snapshot_squares(&json);
        let warm = values.iter().filter(|&&v| v >= CAT_COLD_CM).count();
        assert!(
            warm < 35,
            "mid-game CAT overlay should stay corridor-focused, got {warm} warm squares"
        );
        let e6 = values[square_index(5, 4) as usize];
        let a1 = values[square_index(0, 0) as usize];
        assert!(e6 >= CAT_COLD_CM, "white pawn corridor visible");
        assert!(a1 < CAT_COLD_CM, "far corner cold");
    }

    #[test]
    fn snapshot_does_not_mark_cold_local_counter_noise() {
        let moves = [
            "e2", "e8", "e3", "e7", "e4", "e6", "e3h", "f6h", "c3h", "d6h", "a3h", "h6h", "d4",
            "f6", "g3h", "c4v", "f5v", "d4h", "e5h", "b6h", "a5h", "g4h", "e4", "e6", "f4", "d6",
            "f5", "d5", "h1h", "a7h", "f1h", "c7h", "e5", "f5", "d5", "f4", "d6", "g4", "c6",
            "e7h", "b6", "h4", "a6", "i4", "a7", "i3", "b7", "i2", "c7", "h2",
        ];
        let mut board = Board::new();
        for mv in moves {
            board.apply_algebraic(mv);
        }
        let json = cat_snapshot_json(&mut board);
        let direct = parse_wall_field(&json, "d3v", "directHeat");
        let heat = parse_wall_field(&json, "d3v", "heat");
        assert!(
            direct > 0,
            "fixture d3v keeps its low alternate-route impact"
        );
        assert!(
            heat < CAT_COLD_CM,
            "d3v should not inherit counter heat from lukewarm legal walls, got {heat}; json={json}"
        );
        assert!(
            wall_entry_contains(&json, "d3v", "\"attention\":true"),
            "positive low alternate-route impact should stay visible"
        );
    }

    #[test]
    fn defensive_wall_inherits_hot_neighbor_denial_heat() {
        let moves = [
            "e2", "e8", "e3", "e7", "e4", "e6", "c3h", "f6h", "e3h", "d6h", "g3h", "b6h", "a3h",
            "h6h", "d4", "f6", "f5v", "f5", "e4h", "c4v", "e5h", "e5", "a5h", "a8h", "d5", "d4",
            "g4v", "e4", "d6", "f4", "c6", "g4", "b6", "c8h", "a6",
        ];
        let mut board = Board::new();
        for mv in moves {
            board.apply_algebraic(mv);
        }
        let json = cat_snapshot_json(&mut board);
        let direct = parse_wall_field(&json, "f8h", "directHeat");
        let heat = parse_wall_field(&json, "f8h", "heat");
        assert!(
            direct < CAT_COLD_CM,
            "fixture f8h starts visually cold before denial boost, direct={direct}; json={json}"
        );
        assert!(
            heat >= 510,
            "f8h should inherit 150% hot neighbor denial heat, got {heat}; json={json}"
        );
    }

    fn parse_snapshot_squares(json: &str) -> Vec<u16> {
        let edge = "\"squares\":[";
        let start = json.find(edge).unwrap() + edge.len();
        let end = json.find("],\"reachable\"").unwrap();
        json[start..end]
            .split(',')
            .filter_map(|s| s.trim().parse::<u16>().ok())
            .collect()
    }

    fn wall_entry<'a>(json: &'a str, alg: &str) -> &'a str {
        let needle = format!("{{\"alg\":\"{alg}\"");
        let start = json.find(&needle).expect("wall entry exists");
        let tail = &json[start..];
        let end = tail.find('}').unwrap() + 1;
        &tail[..end]
    }

    fn wall_entry_contains(json: &str, alg: &str, needle: &str) -> bool {
        wall_entry(json, alg).contains(needle)
    }

    fn parse_wall_field(json: &str, alg: &str, key: &str) -> u16 {
        let entry = wall_entry(json, alg);
        let needle = format!("\"{key}\":");
        let start = entry.find(&needle).unwrap() + needle.len();
        let rest = &entry[start..];
        let end = rest
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse().unwrap()
    }
}
