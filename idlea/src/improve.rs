use std::fs::File;
use std::io::Write;
use std::thread;
use std::sync::{mpsc, Arc};
// use std::time::Instant;
use cpu_time::ThreadTime;
use std::collections::HashSet;
use spmc;

use rand::Rng;

use crate::game::{Upgrade, Move, LvlUp, Switch};
use crate::game_state::GameState;
use crate::upg_seq;

const DEPTH_THREADING: usize = 2; // use worker threads for this level of depth or higher
const WT_A: f32 = 0.25; // weights are 2^-(ttl/WT_T) + WT_A
const WT_T: f32 = 60.0 * 60.0; // 1 hour

static mut PUSHY: bool = false;

type Switches = Vec<u32>;

pub unsafe fn set_pushy(do_pushy: bool) {
    PUSHY = do_pushy;
}

pub fn improve_main(
    gs: GameState<'static>, // initial game state
    initial_moves_file: &str,
    output_file: &str,
    max_depth: usize,
    fast: bool,
) {
    let seq = upg_seq::load_sequence(initial_moves_file, &gs);

    let mut scratchpad = GameState::new_from_game(gs.g);

    println!("{} moves in initial sequence", seq.len());
    let initial_points = upg_seq::score(&gs, &mut scratchpad, &seq);
    let initial_score = upg_seq::score_spare(&gs, &mut scratchpad, &seq);
    println!(
        "Initial score is {:.4} with {:.3} hours to spare",
        initial_points, initial_score
    );

    // print_moves(&seq);
    // for mv in &seq {
    //     print!("{} needs ", mv);
    //     print_moves(gs.g.prereqs.get(&mv).unwrap());
    // }
    // let mut variations = upg_seq::Variations::try_seqs(&seq, gs.g);
    // while let Some(new_seq) = variations.next() {
    //     print_moves(new_seq);
    // }
    // return;

    let mut best_score = initial_score;
    let mut best_seq = seq;
    let mut depth = 1;
    let gs = Arc::new(gs);
    let cpus = num_cpus::get();
    let pushy = unsafe { PUSHY };
    loop {
        print!("d{}: ", depth);
        let (new_seq, new_score) = if depth >= DEPTH_THREADING {
            println!("Optimizing with {} threads", cpus);
            find_improvement_threaded(&best_seq, best_score, &gs, true, depth, cpus, fast, pushy)
        } else {
            find_improvement(&best_seq, best_score, &gs, true, depth, fast, pushy)
        };
        if new_score > best_score {
            println!(
                "Found improvement by {:.3} to {:.3}",
                new_score - best_score,
                new_score
            );
            best_seq = new_seq;
            best_score = new_score;
            depth = 1;
            if output_file != "" {
                let mut file = File::create(output_file).unwrap();
                for mv in &best_seq {
                    writeln!(&mut file, "{}", mv.to_string(gs.g)).unwrap();
                }
            }
        } else {
            depth += 1;
            if depth > max_depth {
                break;
            }
        }
        // print!("d{}: ", depth);
        // let resp = find_improvement(best_seq, best_score, gs, true, depth);
        // new_seq = resp.0;
        // new_score = resp.1;
    }
}

#[allow(dead_code)]
fn print_moves(moves: &[Move]) {
    let mut first = true;
    for mv in moves {
        if first {
            print!("{}", mv);
            first = false;
        } else {
            print!(" {}", mv);
        }
    }
    println!();
}

fn find_improvement(
    seq: &[Move],
    seq_score: f64,
    gs: &GameState,
    chatty: bool,
    depth: usize,
    fast: bool,
    pushy: bool,
) -> (Vec<Move>, f64) {
    let mut best_score = seq_score;
    let mut scratchpad = GameState::new_from_game(gs.g);
    let mut best_seq = seq.to_vec();

    let mut variations: Box<dyn upg_seq::VarIter> = if pushy {
        Box::from(upg_seq::VariationsPushy::try_seqs(&seq, &gs.g))
    } else {
        Box::from(upg_seq::Variations::try_seqs(&seq, &gs.g))
    };

    while let Some(new_seq) = variations.next() {
        let mut s = upg_seq::score_spare(gs, &mut scratchpad, new_seq);
        if depth > 1 {
            let (good_seq, good_score) =
                find_improvement(new_seq, s, gs, false, depth - 1, fast, false);
            s = good_score;
            if s > best_score {
                best_seq = good_seq;
            }
        } else {
            if s > best_score {
                best_seq.copy_from_slice(new_seq);
            }
        }
        let next_best = keep_score(s, best_score, seq_score, chatty);
        if next_best > best_score {
            best_score = next_best;
            if fast {
                break;
            }
        }
    }
    if chatty {
        println!();
    }

    (best_seq, best_score)
}

fn keep_score(sc: f64, best_score: f64, seq_score: f64, chatty: bool) -> f64 {
    let mut best_score = best_score;
    if sc > best_score {
        best_score = sc;
        if chatty {
            print!("!");
            // } else {
            //     print!("-")
        }
    } else if sc > seq_score && chatty {
        print!(",");
    } else if chatty {
        print!(".");
    }
    if chatty {
        std::io::stdout().flush().unwrap();
    }
    best_score
}

#[allow(dead_code)]
fn find_improvement_threaded(
    seq: &[Move],
    seq_score: f64,
    gs: &Arc<GameState<'static>>,
    chatty: bool,
    depth: usize,
    cpus: usize,
    fast: bool,
    pushy: bool,
) -> (Vec<Move>, f64) {
    let mut best_score = seq_score;
    let mut scratchpad = GameState::new_from_game(gs.g);
    let mut best_seq = seq.to_vec();

    let mut variations: Box<dyn upg_seq::VarIter> = if pushy {
        Box::from(upg_seq::VariationsPushy::try_seqs(&seq, &gs.g))
    } else {
        Box::from(upg_seq::Variations::try_seqs(&seq, &gs.g))
    };

    let (mut tx_imp, rx_imp): (
        spmc::Sender<(Vec<Move>, f64)>,
        spmc::Receiver<(Vec<Move>, f64)>,
    ) = spmc::channel();
    let (tx_best, rx_best) = mpsc::channel();
    let mut handles = Vec::new();
    // let gsa = Arc::new(gs);
    for _ in 0..cpus {
        let rx_imp = rx_imp.clone();
        let tx_best = mpsc::Sender::clone(&tx_best);
        let gsc = Arc::clone(&gs);
        // let gs_thread: GameState<'static> = GameState::new_from_game(gs.g);
        // gs_thread.copy_from(gs);
        // let gsc = Arc::clone(&gsa);
        handles.push(thread::spawn(move || {
            let gs: &GameState = gsc.as_ref();
            // let (new_seq, sc) = rx_imp.recv().unwrap();
            while let Ok((new_seq, sc)) = rx_imp.recv() {
                let rslt = find_improvement(&new_seq, sc, gs, false, depth - 1, fast, false);
                tx_best.send(rslt).unwrap();
            }
        }));
    }

    while let Some(new_seq) = variations.next() {
        let s = upg_seq::score_spare(gs, &mut scratchpad, new_seq);
        let new_seq = new_seq.to_vec();
        tx_imp.send((new_seq, s)).unwrap();
    }
    drop(tx_imp);

    drop(tx_best);
    for (good_seq, s) in rx_best {
        if s > best_score {
            best_seq = good_seq;
        }
        best_score = keep_score(s, best_score, seq_score, chatty);
    }
    if chatty {
        println!();
    }

    (best_seq, best_score)
}

pub fn improve_main_random(
    gs: GameState<'static>,
    output_file: &str,
    max_depth: usize,
    fast: bool,
    switches: &Switches,
    depth_thr: f64,
) {
    let mut best_score = std::f64::MIN;
    let gs = Arc::new(gs);
    let cpus = num_cpus::get();
    println!("Running in {} worker threads", cpus);

    let (tx, rx) = mpsc::channel();
    let mut _handles = Vec::new();

    for _ in 0..cpus {
        let txc = mpsc::Sender::clone(&tx);
        let gsc = Arc::clone(&gs);
        let sw = switches.clone();
        _handles.push(thread::spawn(move || {
            let start_time = ThreadTime::now();
            let gs: &GameState = gsc.as_ref();
            let mut scratchpad = GameState::new_from_game(gs.g);
            loop {
                let (seq, score) = improved_random(
                    &gs,
                    &mut scratchpad,
                    max_depth,
                    fast,
                    &start_time,
                    &sw,
                    depth_thr,
                );
                txc.send((seq, score)).unwrap();
            }
        }));
    }

    for (seq, score) in rx {
        if score > best_score {
            println!("New best score! {:.4} hours left", score);
            best_score = score;
            if output_file != "" {
                // TODO: write to .temp file, rename
                let temp_file = format!("{}.wtemp", output_file);
                let mut file = File::create(&temp_file).unwrap();
                for mv in &seq {
                    writeln!(&mut file, "{}", mv.to_string(gs.g)).unwrap();
                }
                drop(file);
                std::fs::rename(&temp_file, output_file).unwrap();
            }
        }
    }
}

fn improved_random(
    gs: &GameState<'static>,
    scratch: &mut GameState,
    max_depth: usize,
    fast: bool,
    start_time: &ThreadTime,
    sw: &Switches,
    depth_thr: f64,
) -> (Vec<Move>, f64) {
    scratch.copy_from(gs);
    scratch.update_rates();
    let mut best_seq = random_play(scratch, sw);
    // println!("Random seq:");
    // for mv in &best_seq {
    //     println!(" {}", mv);
    // }
    let initial_score = upg_seq::score_spare(&gs, scratch, &best_seq);
    let mut best_score = initial_score;
    // println!("Random seq: {:.4} hours spare", initial_score);
    let mut depth = 1;
    let pushy = unsafe { PUSHY };
    loop {
        let (new_seq, new_score) =
            find_improvement(&best_seq, best_score, &gs, false, depth, fast, pushy);
        if new_score > best_score {
            best_seq = new_seq;
            best_score = new_score;
            depth = 1;
        } else {
            if new_score < depth_thr {
                break; // give up on lost causes early
            }
            depth += 1;
            if depth > max_depth {
                break;
            }
        }
    }
    let elapsed = start_time.elapsed().as_secs_f32();
    println!(
        "T={:.6} improved random: initial {:.4} final: {:.4} to spare",
        elapsed, initial_score, best_score
    );
    (best_seq, best_score)
}

fn random_play(gs: &mut GameState, sw: &Switches) -> Vec<Move> {
    let mut seq = Vec::new();
    let g = gs.g;
    let mut rng = rand::thread_rng();
    let mut options = Vec::new();
    let mut weights = Vec::new();
    let mut scheduled = HashSet::new();
    for (uidx, level) in gs.levels.iter().enumerate() {
        for done_lvl in 1..(level + 1) {
            scheduled.insert(Move::LvlUp(LvlUp {
                uidx,
                level: done_lvl,
            }));
        }
    }
    loop {
        // Pretend to play the game
        // Find all valid upgrades
        options.clear();
        for iupg in 0..g.upgrades.len() {
            if gs.levels[iupg] == g.upgrades[iupg].costs().len() {
                continue;
            }
            let ttl = gs.time_till_lvlup(iupg);
            if let Some(ttl) = ttl {
                let level = gs.levels[iupg];
                options.push((
                    Move::LvlUp(LvlUp {
                        uidx: iupg as usize,
                        level: level + 1,
                    }),
                    ttl,
                ));
            }
        }
        if options.len() == 0 {
            break;
        }

        // Give each option its own weight
        weights.clear();
        let mut ttl_wt = 0.0;
        for mv_ttl in &options {
            ttl_wt += WT_A + (-mv_ttl.1 as f32 / WT_T).exp2();
            weights.push(ttl_wt);
        }

        // Pick an option at random
        let choice_wt = rng.gen_range(0.0, ttl_wt);
        let choice = weights.iter().position(|&w| choice_wt < w).unwrap();
        let (mv, ttl) = options.swap_remove(choice);
        gs.advance_time(ttl);
        match &mv {
            Move::LvlUp(lvlup) => {
                gs.level_up(lvlup.uidx);
            }
            Move::Switch(sw) => {
                gs.change_prod(sw);
            }
        }
        // println!("  Adding move {}", mv);
        scheduled.insert(mv);
        seq.push(mv);
    }

    // Now add any unfulfilled upgrades so the optimizer might find a time for them
    let mut extras = Vec::new();
    for (uidx, upg) in g.upgrades.iter().enumerate() {
        for level in (gs.levels[uidx] + 1)..(upg.costs().len() + 1) {
            let mv = Move::LvlUp(LvlUp { uidx, level });
            extras.push(mv);
        }
    }
    while extras.len() > 0 {
        let choice = rng.gen_range(0, extras.len());
        // println!("  Adding extra move {}", extras[choice]);
        if g.prereqs[&extras[choice]]
            .iter()
            .all(|mv| scheduled.contains(mv))
        {
            // println!(
            //     "YAY move {:?} has prereqs {:?} which are in {:?}",
            //     extras[choice], g.prereqs[&extras[choice]], scheduled
            // );
            scheduled.insert(extras[choice]);
            seq.push(extras.swap_remove(choice));
            // } else {
            //     println!(
            //         "BOO move {:?} has prereqs {:?} which are not in {:?}",
            //         extras[choice], g.prereqs[&extras[choice]], scheduled
            //     );
        }
    }

    for (uidx, &num_sw) in sw.iter().enumerate() {
        if num_sw == 0 {
            continue;
        }
        if let Upgrade::Producer(up) = &g.upgrades[uidx] {
            if up.produces2.len() == 0 {
                panic!(
                    "No production switches possible for upgrade {}: {}",
                    uidx, up.name
                );
            }
        } else {
            panic!(
                "No production switches possible for upgrade {}: non-producer",
                uidx
            );
        }
        // let mut min_idx = seq.len();
        let mut min_idx = 0;
        for (iseq, &mv) in seq.iter().enumerate() {
            if mv == Move::LvlUp(LvlUp { uidx, level: 1 }) {
                min_idx = iseq + 1;
            }
        }
        // if min_idx == seq.len() {
        //     println!("seq = {:?}", seq);
        //     panic!("Cannot find level 1 upgrade for {}", uidx);
        // }
        let mut indices: Vec<usize> = (0..num_sw)
            .map(|_| rng.gen_range(min_idx, seq.len() + 1))
            .collect();
        indices.sort();
        for (i, seq_idx) in indices.iter().enumerate() {
            seq.insert(
                seq_idx + i,
                Move::Switch(Switch {
                    uidx,
                    iprod: (i + 1) % 2,
                }),
            );
        }
    }

    seq
}

pub fn switches_from_arg(swarg: &str) -> Switches {
    let mut sw = Switches::new();
    for iupg in swarg.split(",") {
        if iupg != "" {
            sw.push(iupg.parse::<u32>().unwrap());
        }
    }
    sw
}
