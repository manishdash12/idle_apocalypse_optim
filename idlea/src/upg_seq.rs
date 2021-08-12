use std::fs::File;
use hashbrown::{HashMap, HashSet};
// use num_iter;
// use streaming_iterator::StreamingIterator;

use crate::game::{Game, Upgrade, Move, LvlUp, Switch};
use crate::game_state::GameState;

pub fn load_sequence(csv_file: &str, gs: &GameState) -> Vec<Move> {
    let file = File::open(csv_file).unwrap();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(file);
    let mut recs = rdr.records();

    let mut switch_moves = HashMap::new();
    for (uidx, upg) in gs.g.upgrades.iter().enumerate() {
        if let Upgrade::Producer(prod) = upg {
            if prod.prod_names.0 != "" {
                let sw = Switch { uidx, iprod: 0 };
                switch_moves.insert(sw.to_string(gs.g), sw);
                let sw = Switch { uidx, iprod: 1 };
                switch_moves.insert(sw.to_string(gs.g), sw);
            }
        }
    }
    for (k, v) in switch_moves.iter() {
        println!("{}: {}~{}", k, v.uidx, v.iprod);
    }

    let mut moves = Vec::new();
    let mut levels = gs.levels.to_vec();

    let header = recs.next().unwrap().unwrap();
    let mut upg_idx_col: usize = 0;
    if header.len() == 1 {
        let uidx: usize = header[0].parse().unwrap();
        levels[uidx] += 1;
        moves.push(Move::LvlUp(LvlUp {
            uidx,
            level: levels[uidx],
        }));
    } else {
        upg_idx_col = header.iter().position(|s| s == "upg #").unwrap();
    }

    for row in recs {
        let svalue = &row.as_ref().unwrap()[upg_idx_col];
        if let Some(sw) = switch_moves.get(svalue) {
            moves.push(Move::Switch(*sw));
            continue;
        }
        let uidxr: Result<i32, _> = svalue.parse();
        match uidxr {
            Ok(uidx) => {
                if uidx >= 0 {
                    levels[uidx as usize] += 1;
                    moves.push(Move::LvlUp(LvlUp {
                        uidx: uidx as usize,
                        level: levels[uidx as usize],
                    }));
                } else {
                    println!("Removing final move {}", uidx);
                }
            }
            Err(_) => {
                println!(
                    "Cannot parse {:?} in column {} as usize",
                    svalue, upg_idx_col
                );
            }
        };
    }

    moves
}

pub fn score(gs: &GameState, scratch: &mut GameState, seq: &[Move]) -> f64 {
    scratch.copy_from(gs);
    scratch.update_rates();
    for mv in seq {
        // println!("{}: t={}, pt_rate={} points={}",
        //     mv, scratch.time, scratch.pt_rate, scratch.points);
        match mv {
            Move::LvlUp(lvlup) => {
                let ttl = scratch.time_till_lvlup(lvlup.uidx);
                match ttl {
                    Some(t) => {
                        let t = t.ceil() + 1.0;
                        scratch.advance_time(t);
                        scratch.level_up(lvlup.uidx);
                    }
                    None => break,
                }
            }
            Move::Switch(sw) => {
                scratch.change_prod(sw);
            }
        }
    }
    scratch.finish();
    scratch.points
}

pub fn score_spare(gs: &GameState, scratch: &mut GameState, seq: &[Move]) -> f64 {
    scratch.copy_from(gs);
    scratch.update_rates();
    for mv in seq {
        // println!("{}: t={}, pt_rate={} points={}",
        //     mv, scratch.time, scratch.pt_rate, scratch.points);
        match mv {
            Move::LvlUp(lvlup) => {
                let ttl = scratch.time_till_lvlup(lvlup.uidx);
                match ttl {
                    Some(t) => {
                        let t = t.ceil() + 1.0;

                        if scratch.points + scratch.pt_rate * t > scratch.g.goal
                            && scratch.pt_rate > 0.0
                        {
                            let dt_win = (scratch.g.goal - scratch.points) / scratch.pt_rate;
                            let spare = scratch.g.event_time - (scratch.time + dt_win);
                            // println!("Early win by {} seconds", spare);
                            return spare / 60.0 / 60.0;
                        }

                        scratch.advance_time(t);
                        scratch.level_up(lvlup.uidx);
                    }
                    None => break,
                }
            }
            Move::Switch(sw) => {
                scratch.change_prod(sw);
            }
        }
    }
    scratch.finish();
    let spare = if scratch.pt_rate == 0.0 {
        -1.0e30
    } else {
        -(scratch.g.goal - scratch.points) / scratch.pt_rate
    };
    // println!("goal={} pts={} rate={} spare={}", scratch.g.goal, scratch.points, scratch.pt_rate, spare);
    spare / 60. / 60.
}

pub trait VarIter {
    fn next(&mut self) -> Option<&[Move]>;
}

pub struct Variations<'a> {
    orig_seq: &'a [Move],
    seq: Vec<Move>,
    g: &'a Game,
    iup: usize,
    iup_iter: std::ops::Range<usize>,
    //adv_iter: Option<num_iter::RangeStep<i32>>,
    adv_iter: std::iter::Rev<std::ops::Range<usize>>,
    post_iter: std::ops::Range<usize>,
    clean_seq: bool,
}

impl Variations<'_> {
    pub fn try_seqs<'a>(seq: &'a [Move], game: &'a Game) -> Variations<'a> {
        // let mut work = Vec::new();
        // work.extend_from_slice(seq);
        Variations {
            orig_seq: seq,
            seq: seq.to_vec(),
            g: game,
            iup: 0,
            iup_iter: (1..seq.len()),
            adv_iter: (0..0).rev(),
            post_iter: 0..0,
            clean_seq: true,
        }
    }
}

impl VarIter for Variations<'_> {
    // In principle this could implement StreamingIterator if there's much benefit.
    fn next(&mut self) -> Option<&[Move]> {
        loop {
            if let Some(iprev) = self.adv_iter.next() {
                let mv = self.seq[iprev + 1];
                let prereqs = self.g.prereqs.get(&mv);
                if {
                    if let Some(prereqs) = prereqs {
                        if prereqs.contains(&self.seq[iprev]) {
                            // can't go further
                            self.adv_iter = (0..0).rev();
                            false // try from post_iter noq
                        } else {
                            true
                        } // ok to swap & yield
                    } else {
                        true
                    } // ok
                } {
                    // println!("Preponing {}", mv);
                    self.seq[iprev + 1] = self.seq[iprev];
                    self.seq[iprev] = mv;
                    self.clean_seq = false;
                    return Some(&self.seq);
                }
            }

            if let Some(inext) = self.post_iter.next() {
                if inext == self.iup + 1 && !self.clean_seq {
                    // println!("Restoring seq, finding postponements");
                    self.seq.copy_from_slice(self.orig_seq); // first time in this section
                    self.clean_seq = true;
                }
                let mv = self.seq[inext - 1];
                let prereqs = self.g.prereqs.get(&self.seq[inext]);
                if {
                    if let Some(prereqs) = prereqs {
                        if prereqs.contains(&mv) {
                            // can't go further
                            self.post_iter = 0..0;
                            false // fall through to iup_iter.next()
                        } else {
                            true
                        } // ok to swap & yield
                    } else {
                        true
                    } // ok
                } {
                    // println!("Postponing {}", mv);
                    self.seq[inext - 1] = self.seq[inext];
                    self.seq[inext] = mv;
                    self.clean_seq = false;
                    // speedup: first iteration here is always redundant
                    if inext == self.iup + 1 {
                        continue; // do the swap but don't emit
                    }
                    return Some(&self.seq);
                }
            }

            self.iup = self.iup_iter.next()?; // Returns None if iup range has terminated
                                              // self.adv_iter = Some(num_iter::range_step((self.iup-1) as i32, 0, -1));
            self.adv_iter = (1..self.iup).rev();
            self.post_iter = self.iup + 1..self.seq.len();
            // println!("Restorinq seq, iup = {}", self.iup);
            if !self.clean_seq {
                self.seq.copy_from_slice(self.orig_seq);
                self.clean_seq = true;
            }
        }
    }
}

// impl StreamingIterator for Variations<'_> {
//     type Item<'a> = &'a [Move];
//
//     fn next(&'a mut self) -> Option<Self::Item<'a>> {
//         if self.adv_iter.is_none() && self.post_iter.is_none() {
//             self.iup = self.iup_iter.next()?;
//         }
//         // Some(num_iter.range_step())
//         Some(&self.seq)
//     }
// }

pub struct VariationsPushy<'a> {
    orig_seq: &'a [Move],
    seq: Vec<Move>,
    g: &'a Game,
    iup: usize,
    iup_iter: std::ops::Range<usize>,
    //adv_iter: Option<num_iter::RangeStep<i32>>,
    adv_iter: std::iter::Rev<std::ops::Range<usize>>,
    post_iter: std::ops::Range<usize>,
    clean_seq: bool,
    mvs: Vec<Move>,
    // prerequisete of all moves advanced, or set of all moves being postponed:
    prereqs: HashSet<Move>,
}

impl VariationsPushy<'_> {
    pub fn try_seqs<'a>(seq: &'a [Move], game: &'a Game) -> VariationsPushy<'a> {
        // let mut work = Vec::new();
        // work.extend_from_slice(seq);
        VariationsPushy {
            orig_seq: seq,
            seq: seq.to_vec(),
            g: game,
            iup: 0,
            iup_iter: (1..seq.len()),
            adv_iter: (0..0).rev(),
            post_iter: 0..0,
            clean_seq: true,
            mvs: Vec::new(),
            prereqs: HashSet::new(),
        }
    }
}

impl VarIter for VariationsPushy<'_> {
    fn next(&mut self) -> Option<&[Move]> {
        loop {
            if let Some(iprev) = self.adv_iter.next() {
                let prev_mv = &self.seq[iprev];
                if self.prereqs.contains(prev_mv) {
                    self.mvs.insert(0, *prev_mv);
                    if let Some(prereqs) = self.g.prereqs.get(prev_mv) {
                        for p in prereqs {
                            self.prereqs.insert(*p);
                        }
                    }
                    continue;
                }
                self.seq[iprev + self.mvs.len()] = *prev_mv;
                self.seq[iprev..iprev + self.mvs.len()].copy_from_slice(&self.mvs);
                self.clean_seq = false;
                return Some(&self.seq);
            }

            if let Some(inext) = self.post_iter.next() {
                if inext == self.iup + 1 {
                    // println!("Restoring seq, finding postponements");
                    if !self.clean_seq {
                        self.seq.copy_from_slice(self.orig_seq); // first time in this section
                        self.clean_seq = true;
                    }
                    self.mvs.clear();
                    let mv = self.seq[self.iup];
                    self.mvs.push(mv);
                    self.prereqs.clear();
                    self.prereqs.insert(mv); // not prereqs here, actually a set of moves postponed
                }
                let next_mv = &self.seq[inext];
                if let Some(prereqs) = self.g.prereqs.get(next_mv) {
                    let mut must_push = false;
                    for mv in prereqs {
                        if self.prereqs.contains(mv) {
                            must_push = true;
                            break;
                        }
                    }
                    if must_push {
                        self.mvs.push(*next_mv);
                        self.prereqs.insert(*next_mv);
                        continue;
                    }
                }
                // postpone self.mvs by one spot
                self.seq[inext - self.mvs.len()] = *next_mv;
                self.seq[inext - self.mvs.len() + 1..inext + 1].copy_from_slice(&self.mvs);
                self.clean_seq = false;
                // speedup: first iteration here is always redundant
                if inext == self.iup + 1 {
                    continue; // do the swap but don't emit
                }
                return Some(&self.seq);
            }

            self.iup = self.iup_iter.next()?; // Returns None if iup range has terminated
                                              // self.adv_iter = Some(num_iter::range_step((self.iup-1) as i32, 0, -1));
            self.adv_iter = (1..self.iup).rev();
            self.post_iter = self.iup + 1..self.seq.len();
            // println!("Restorinq seq, iup = {}", self.iup);

            // Initialization at start of advance sequence:
            if !self.clean_seq {
                self.seq.copy_from_slice(self.orig_seq);
                self.clean_seq = true;
            }
            self.mvs.clear();
            self.prereqs.clear();
            let mv = self.seq[self.iup];
            if let Some(prereqs) = self.g.prereqs.get(&mv) {
                for p in prereqs {
                    self.prereqs.insert(*p);
                }
            }
            self.mvs.push(mv);
        }
    }
}
