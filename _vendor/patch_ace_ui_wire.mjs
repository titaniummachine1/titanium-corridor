import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

function read(rel) {
  return fs.readFileSync(path.join(root, rel), 'utf8');
}

function write(rel, text) {
  fs.writeFileSync(path.join(root, rel), text, 'utf8');
  console.log('wrote', rel);
}

// --- search.rs ---
let search = read('engine/src/ace/search.rs');

const thinkResultOld = `pub struct ThinkResult {
    pub mv: i16,
    pub score: i32,
    pub depth: i32,
    pub nodes: u64,
    pub ms: u64,
}`;

const thinkResultNew = `#[derive(Debug, Clone)]
pub struct AceDepthLogEntry {
    pub depth: i32,
    pub score: i32,
    pub nodes: u64,
    pub elapsed_ms: u64,
    pub marginal_nodes: u64,
    pub pv: String,
}

pub struct ThinkResult {
    pub mv: i16,
    pub score: i32,
    pub depth: i32,
    pub nodes: u64,
    pub ms: u64,
    pub white_dist: u8,
    pub black_dist: u8,
    pub depth_log: Vec<AceDepthLogEntry>,
}

fn emit_ace_progress(
    engine_label: &str,
    depth_log: &[AceDepthLogEntry],
    search_depth: i32,
    nodes: u64,
    root_score: i32,
    white_dist: u8,
    black_dist: u8,
    elapsed_ms: u64,
) {
    let mut depth_json = String::new();
    for (i, e) in depth_log.iter().enumerate() {
        if i > 0 {
            depth_json.push(',');
        }
        let pv = e.pv.replace('\\\\', "\\\\\\\\").replace('"', "\\\\\\"");
        depth_json.push_str(&format!(
            "{{\\"depth\\":{},\\"score\\":{},\\"nodes\\":{},\\"elapsedMs\\":{},\\"marginalNodes\\":{},\\"pv\\":\\"{}\\"}}",
            e.depth, e.score, e.nodes, e.elapsed_ms, e.marginal_nodes, pv
        ));
    }
    eprintln!(
        "info json {{\\"engine\\":\\"{}\\",\\"stoppedBy\\":\\"{}\\",\\"searchDepth\\":{},\\"nodes\\":{},\\"rootScore\\":{},\\"whiteDist\\":{},\\"blackDist\\":{},\\"elapsedMs\\":{},\\"depthLog\\":[{}]}}",
        engine_label,
        engine_label,
        search_depth,
        nodes,
        root_score,
        white_dist,
        black_dist,
        elapsed_ms,
        depth_json
    );
    let _ = std::io::Write::flush(&mut std::io::stderr());
}`;

if (!search.includes(thinkResultOld)) throw new Error('ThinkResult block not found');
search = search.replace(thinkResultOld, thinkResultNew);

search = search.replace(
  '    pub fn think(&mut self, time_ms: u64, max_depth: i32, full: bool) -> ThinkResult {',
  `    pub fn think(
        &mut self,
        time_ms: u64,
        max_depth: i32,
        full: bool,
        log: bool,
        engine_label: &str,
    ) -> ThinkResult {`,
);

search = search.replace(
  `        let mut stable = 0;
        let max_depth = if max_depth > 0 { max_depth } else { 30 };

        for d in 1..=max_depth {
            let result = if d >= 4 && last_score > -2000 && last_score < 2000 {`,
  `        let mut stable = 0;
        let mut depth_log: Vec<AceDepthLogEntry> = Vec::new();
        let max_depth = if max_depth > 0 { max_depth } else { 30 };

        for d in 1..=max_depth {
            let nodes_at_depth = self.nodes;
            let result = if d >= 4 && last_score > -2000 && last_score < 2000 {`,
);

const okBlockOld = `                Ok(sc) => {
                    stable = if self.root_best == last_best { stable + 1 } else { 0 };
                    last_best = self.root_best;
                    last_score = sc;
                    last_depth = d;
                    if sc > MATE - 200 || sc < -(MATE - 200) {
                        break; // forced result
                    }`;

const okBlockNew = `                Ok(sc) => {
                    stable = if self.root_best == last_best { stable + 1 } else { 0 };
                    last_best = self.root_best;
                    last_score = sc;
                    last_depth = d;
                    let elapsed_ms = t0.elapsed().as_millis() as u64;
                    let pv = if last_best != 0 {
                        super::ace_to_algebraic(last_best)
                    } else {
                        String::new()
                    };
                    depth_log.push(AceDepthLogEntry {
                        depth: d,
                        score: last_score,
                        nodes: self.nodes,
                        elapsed_ms,
                        marginal_nodes: self.nodes.saturating_sub(nodes_at_depth),
                        pv,
                    });
                    if log {
                        self.refresh_dist(0);
                        let white_dist = self.d0[self.dist0_idx][self.g.pawn[0]];
                        let black_dist = self.d1[self.dist1_idx][self.g.pawn[1]];
                        emit_ace_progress(
                            engine_label,
                            &depth_log,
                            d,
                            self.nodes,
                            last_score,
                            white_dist,
                            black_dist,
                            elapsed_ms,
                        );
                    }
                    if sc > MATE - 200 || sc < -(MATE - 200) {
                        break; // forced result
                    }`;

if (!search.includes(okBlockOld)) throw new Error('Ok block not found');
search = search.replace(okBlockOld, okBlockNew);

search = search.replace(
  `        ThinkResult {
            mv: last_best,
            score: last_score,
            depth: last_depth,
            nodes: self.nodes,
            ms: t0.elapsed().as_millis() as u64,
        }`,
  `        self.refresh_dist(0);
        let white_dist = self.d0[self.dist0_idx][self.g.pawn[0]];
        let black_dist = self.d1[self.dist1_idx][self.g.pawn[1]];
        let ms = t0.elapsed().as_millis() as u64;

        ThinkResult {
            mv: last_best,
            score: last_score,
            depth: last_depth,
            nodes: self.nodes,
            ms,
            white_dist,
            black_dist,
            depth_log,
        }`,
);

write('engine/src/ace/search.rs', search);

// --- mod.rs ---
let modrs = read('engine/src/ace/mod.rs');
modrs = modrs.replace(
  `    pub ti_movegen: bool,
}`,
  `    pub ti_movegen: bool,
    /// Stream iterative-deepening progress on stderr (\`info json\`).
    pub log: bool,
}`,
);
modrs = modrs.replace(
  `            ti_movegen: false,
        }
    }
}`,
  `            ti_movegen: false,
            log: false,
        }
    }
}`,
);
modrs = modrs.replace(
  'pub fn ace_genmove(moves: &[String], params: AceParams) -> Option<(String, ThinkResult)> {',
  'pub fn ace_genmove(\n    moves: &[String],\n    params: AceParams,\n    engine_label: &str,\n) -> Option<(String, ThinkResult)> {',
);
modrs = modrs.replace(
  '    let result = search.think(params.time_ms, params.max_depth, params.full);',
  `    let result = search.think(
        params.time_ms,
        params.max_depth,
        params.full,
        params.log,
        engine_label,
    );`,
);
write('engine/src/ace/mod.rs', modrs);

// --- main.rs ---
let mainrs = read('engine/src/main.rs');
mainrs = mainrs.replace(
  `        } else if arg == "--full" {
            params.full = true;
            i += 1;
            continue;
        } else if arg == "--engine" {`,
  `        } else if arg == "--full" {
            params.full = true;
            i += 1;
            continue;
        } else if arg == "--log" {
            params.log = true;
            i += 1;
            continue;
        } else if arg == "--engine" {`,
);
mainrs = mainrs.replace(
  '    match titanium::ace::ace_genmove(&moves, params) {',
  '    match titanium::ace::ace_genmove(&moves, params, label) {',
);
mainrs = mainrs.replace(
  `        Some((algebraic, info)) => {
            eprintln!(
                "info json {{\\"engine\\":\\"{}\\",\\"rootScore\\":{},\\"searchDepth\\":{},\\"nodes\\":{},\\"elapsedMs\\":{}}}",
                label, info.score, info.depth, info.nodes, info.ms
            );
            println!("bestmove {}", algebraic);
        }`,
  `        Some((algebraic, info)) => {
            if !params.log {
                let mut depth_json = String::new();
                for (i, e) in info.depth_log.iter().enumerate() {
                    if i > 0 {
                        depth_json.push(',');
                    }
                    let pv = e.pv.replace('\\\\', "\\\\\\\\").replace('"', "\\\\\\"");
                    depth_json.push_str(&format!(
                        "{{\\"depth\\":{},\\"score\\":{},\\"nodes\\":{},\\"elapsedMs\\":{},\\"marginalNodes\\":{},\\"pv\\":\\"{}\\"}}",
                        e.depth, e.score, e.nodes, e.elapsed_ms, e.marginal_nodes, pv
                    ));
                }
                eprintln!(
                    "info json {{\\"engine\\":\\"{}\\",\\"stoppedBy\\":\\"{}\\",\\"searchDepth\\":{},\\"nodes\\":{},\\"rootScore\\":{},\\"whiteDist\\":{},\\"blackDist\\":{},\\"elapsedMs\\":{},\\"depthLog\\":[{}]}}",
                    label,
                    label,
                    info.depth,
                    info.nodes,
                    info.score,
                    info.white_dist,
                    info.black_dist,
                    info.ms,
                    depth_json
                );
            }
            println!("bestmove {}", algebraic);
        }`,
);
mainrs = mainrs.replace(
  '    let r = search.think(1_000_000_000, depth, true);',
  '    let r = search.think(1_000_000_000, depth, true, false, "ace-bench");',
);
write('engine/src/main.rs', mainrs);

console.log('done');
