use crate::game::{Game, Upgrade, Switch};

#[derive(Debug)]
pub struct GameState<'a> {
    pub levels: Vec<usize>,
    pub res_amt: Vec<f64>,
    pub res_rate: Vec<f64>,
    pub pt_rate: f64,
    pub time: f64,
    pub points: f64,
    pub prod2: Vec<bool>, // true if switched to 2nd production
    pub bonuses: Vec<i32>,
    pub gem_boost: i32,
    pub commercial_mod: f64,
    pub g: &'a Game,
}

impl GameState<'_> {
    pub fn set_gem_boost(&mut self, bonus: i32) {
        self.gem_boost = bonus;
    }

    pub fn new_from_game<'a>(game: &'a Game) -> GameState<'a> {
        let mut gs = GameState {
            levels: vec![0; game.upgrades.len()],
            res_amt: vec![0.; game.nres],
            res_rate: vec![0.; game.nres],
            pt_rate: 0.,
            time: 0.,
            points: 0.,
            prod2: vec![false; game.upgrades.len()],
            bonuses: vec![0; game.nres],
            gem_boost: 0,
            commercial_mod: -0.25,
            g: game,
        };
        for (iupg, upg) in game.upgrades.iter().enumerate() {
            let mut is_free = true;
            for cost in upg.costs()[0].iter() {
                if *cost > 0 {
                    is_free = false;
                    break;
                }
            }
            if is_free {
                gs.levels[iupg] = 1;
            }
        }
        gs
    }

    pub fn update_rates(&mut self) {
        for r in &mut self.res_rate {
            *r = 0.;
        }
        for b in &mut self.bonuses {
            *b = self.gem_boost;
        }
        let mut pt_mult = 0.; // sum of fractional point multipliers
        let mut time_fact = 1. + self.commercial_mod;
        for (idx, upg) in self.g.upgrades.iter().enumerate() {
            if let Upgrade::Boost(boost) = upg {
                let level = self.levels[idx];
                if level < 1 {
                    continue;
                }
                pt_mult += boost.pt_mult[level - 1];
                time_fact += boost.time_mod[level - 1];
                for (ires, addtnl) in boost.res_bonus[level - 1].iter().enumerate() {
                    self.bonuses[ires] += addtnl;
                }
            }
        }
        // println!("pt_mult = {}, time_fact = {}, bonuses = {:?}", pt_mult, time_fact, self.bonuses);
        self.pt_rate = 0.;
        for (idx, upg) in self.g.upgrades.iter().enumerate() {
            if let Upgrade::Producer(prod) = upg {
                let level = self.levels[idx];
                if level < 1 {
                    continue;
                }
                let produces = if self.prod2[idx] {
                    &prod.produces2[level - 1]
                } else {
                    &prod.produces[level - 1]
                };
                for (ires, amt) in produces.iter().enumerate() {
                    let net_amt = if *amt > 0 {
                        (*amt + self.bonuses[ires]) as f64
                    } else {
                        *amt as f64 // bonuses do not apply to costs
                    };
                    // println!("  {} adds {:.2} to {}/min", upg.get_name(), net_amt/prod.spawn_time, self.g.res_names[ires]);
                    self.res_rate[ires] += net_amt / prod.spawn_time;
                }
                self.pt_rate += prod.get_pt_rate(level, self.prod2[idx]);
                // println!("  pt_rate is now {}", self.pt_rate);
            }
        }
        self.pt_rate *= 1.0 + pt_mult;
        self.pt_rate /= time_fact;
        for rt in &mut self.res_rate {
            *rt /= time_fact;
        }
    }

    pub fn print_status(&self) {
        println!("Status at t={}", self.time);
        for (ires, res) in self.g.res_names.iter().enumerate() {
            println!(
                "   {}: {:.0}  {:.2}/min",
                res,
                self.res_amt[ires],
                self.res_rate[ires] * 60.
            );
        }
        println!(
            "   {}: {:.0}  {:.2}/min",
            self.g.points_name,
            self.points,
            self.pt_rate * 60.
        );
    }

    pub fn print_levels(&self) {
        println!("Upgrades:");
        for (iup, upg) in self.g.upgrades.iter().enumerate() {
            if self.levels[iup] > 0 {
                println!("   {}: {}", upg.get_name(), self.levels[iup]);
            }
        }
    }

    pub fn time_till_lvlup(&self, iup: usize) -> Option<f64> {
        let up = &self.g.upgrades[iup];
        let nxt_lvl = self.levels[iup] + 1;
        if nxt_lvl > up.costs().len() {
            panic!("Invalid upgrade uidx={} to level {}", iup, nxt_lvl);
            // return None;
        }

        if let Upgrade::Producer(prod) = up {
            if let Some(iup_prev) = prod.needs {
                if self.levels[iup_prev] == 0 {
                    // panic!("Invalid upgrade uidx={}, previous not upgraded", iup);
                    return None;
                }
            }
        }

        // Do we have enough already?
        let cost = &up.costs()[nxt_lvl - 1];
        // println!("costs: {:?}", cost);
        // println!("res_rates: {:?}", self.res_rate);
        if cost
            .iter()
            .zip(self.res_amt.iter())
            .all(|(&c, &h)| (c as f64) <= h)
        {
            return Some(0.0); // Already have what we need
        }

        let mut max_t: f64 = 0.;
        let mut min_t = std::f64::MAX;
        for (ires, have) in self.res_amt.iter().enumerate() {
            let need = (cost[ires] as f64 - have).ceil();
            let rt = self.res_rate[ires];
            if need > 0.0 && rt >= 0.0 {
                if rt > 0.0 {
                    let t = need / rt;
                    max_t = max_t.max(t);
                } else {
                    return None;
                }
            }
            if rt < 0.0 {
                // need to check for running out of this resource
                // min_t = min_t.min(-have / rt);
                // need to check for running low on demand for this resource
                min_t = min_t.min(need / rt);
                // println!("rate={} have={} min_t={}", rt, have, min_t);
            }
        }
        if max_t > self.g.event_time - self.time {
            return None;
        }
        if max_t + self.g.overshoot + 2.0 > min_t {
            return None; // will run out of this resource first
        }

        Some(max_t + self.g.overshoot)
    }

    pub fn advance_time(&mut self, mut dt: f64) {
        if self.time + dt > self.g.event_time {
            dt = self.g.event_time - self.time;
            self.time = self.g.event_time;
        } else {
            self.time += dt;
        }
        self.points += dt * self.pt_rate;
        for (amt, rt) in self.res_amt.iter_mut().zip(self.res_rate.iter()) {
            *amt += dt * *rt;
        }
    }

    pub fn finish(&mut self) {
        let t_remaining = self.g.event_time - self.time;
        if t_remaining > 0.0 {
            self.advance_time(t_remaining);
        }
    }

    pub fn level_up(&mut self, iupg: usize) {
        assert!(iupg < self.levels.len(), "Invalid upgrade index {}", iupg);
        let ilvl = self.levels[iupg];
        assert!(
            ilvl < self.g.upgrades[iupg].costs().len(),
            "Upgrade {} is at max level already",
            self.g.upgrades[iupg].get_name()
        );
        self.levels[iupg] += 1;
        let mut broke = false;
        for (ires, cost) in self.g.upgrades[iupg].costs()[ilvl].iter().enumerate() {
            self.res_amt[ires] -= *cost as f64;
            if self.res_amt[ires] < -5.0 {
                // giving some leeway here
                broke = true;
            }
        }
        if broke {
            println!("Cost exceeds resources which are now {:?}", self.res_amt);
            println!("cost was {:?}", self.g.upgrades[iupg].costs()[ilvl].iter());
        }
        self.update_rates();
    }

    pub fn copy_from(&mut self, src: &GameState) {
        assert_eq!(self.g as *const _, src.g as *const _);
        self.levels.copy_from_slice(&src.levels);
        self.res_amt.copy_from_slice(&src.res_amt);
        self.res_rate.copy_from_slice(&src.res_rate);
        self.bonuses.copy_from_slice(&src.bonuses);
        self.pt_rate = src.pt_rate;
        self.time = src.time;
        self.points = src.points;
        self.prod2.copy_from_slice(&src.prod2);
        self.gem_boost = src.gem_boost;
        self.commercial_mod = src.commercial_mod;
    }

    pub fn change_prod(&mut self, sw: &Switch) {
        self.prod2[sw.uidx] = sw.iprod != 0;
        self.update_rates();
    }
}
