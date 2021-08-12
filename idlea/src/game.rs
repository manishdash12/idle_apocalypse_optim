// use crate::game_state::GameState;
//use std::collections::HashMap;
use hashbrown::hash_map::HashMap;

#[derive(Debug)]
pub struct Game {
    pub name: String,
    pub res_names: Vec<String>,
    pub upgrades: Vec<Upgrade>,
    pub event_time: f64,
    pub points_name: String,
    pub nres: usize,
    pub overshoot: f64,
    pub res_name_len: usize,
    pub upg_name_len: usize,
    pub goal: f64,
    pub prereqs: HashMap<Move, Vec<Move>>,
}

#[derive(Debug)]
pub enum Upgrade {
    Producer(Producer),
    Boost(Boost),
}

#[derive(Debug)]
pub struct Producer {
    pub name: String,
    pub produces: Vec<Vec<i32>>,
    pub produces2: Vec<Vec<i32>>,
    pub spawn_time: f64,
    pub costs: Vec<Vec<i32>>,
    pub points: Vec<f64>,
    pub points2: Vec<f64>,
    pub needs: Option<usize>, // upgrade index necessary to unlock this one
    pub prod_names: (String, String),
}

#[derive(Debug)]
pub struct Boost {
    pub name: String,
    pub res_bonus: Vec<Vec<i32>>,
    pub pt_mult: Vec<f64>,
    pub time_mod: Vec<f64>,
    pub costs: Vec<Vec<i32>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Move {
    LvlUp(LvlUp),
    Switch(Switch),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LvlUp {
    pub uidx: usize,
    pub level: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Switch {
    pub uidx: usize,
    pub iprod: usize, // 0 or 1
}

impl Game {
    pub fn new() -> Game {
        Game {
            name: String::from(""),
            res_names: Vec::new(),
            upgrades: Vec::new(),
            event_time: 3. * 24. * 60. * 60.,
            points_name: String::from("points"),
            nres: 0,
            overshoot: 20.,
            res_name_len: 0,
            upg_name_len: 0,
            goal: 0.,
            prereqs: HashMap::new(),
        }
    }

    pub fn set_resources(&mut self, res_names: &[&str]) {
        self.res_names = res_names.iter().map(|&s| s.to_owned()).collect();
    }

    pub fn add_upgrade(&mut self, upg: Upgrade) {
        self.upgrades.push(upg);
        if self.upgrades.len() == 1 {
            self.upg_name_len = self.upgrades.last().unwrap().get_name().len();
        }
    }

    pub fn find_prereqs(&mut self) {
        let mut pr = HashMap::new();
        let mut first_producer = vec![-1; self.nres];
        for (iupg, upg) in self.upgrades.iter().enumerate() {
            if let Upgrade::Producer(prod) = upg {
                for (ires, amt) in prod.produces[0].iter().enumerate() {
                    if *amt > 0 && first_producer[ires] < 0 {
                        first_producer[ires] = iupg as i32;
                    }
                }
            }
        }

        for (iupg, upg) in self.upgrades.iter().enumerate() {
            for (ilvl, costs) in upg.costs().iter().enumerate() {
                if ilvl == 0 {
                    if let Upgrade::Producer(_) = upg {
                        if iupg > 0 {
                            // Need to unlock previous producer to unlock this one
                            pr.entry(Move::LvlUp(LvlUp {
                                uidx: iupg,
                                level: 1,
                            }))
                            .or_insert_with(Vec::new)
                            .push(Move::LvlUp(LvlUp {
                                uidx: iupg - 1,
                                level: 1,
                            }));
                        }
                    }
                } else {
                    // Need to upgrade to previous level first
                    pr.entry(Move::LvlUp(LvlUp {
                        uidx: iupg,
                        level: ilvl + 1,
                    }))
                    .or_insert_with(Vec::new)
                    .push(Move::LvlUp(LvlUp {
                        uidx: iupg,
                        level: ilvl,
                    }));
                }
                for (ires, amt) in costs.iter().enumerate().rev() {
                    if *amt > 0 {
                        // Need to produce this resource to do the upgrade
                        let prs = pr
                            .entry(Move::LvlUp(LvlUp {
                                uidx: iupg,
                                level: ilvl + 1,
                            }))
                            .or_insert_with(Vec::new);
                        let mv = Move::LvlUp(LvlUp {
                            uidx: first_producer[ires] as usize,
                            level: 1,
                        });
                        if !prs.contains(&mv) {
                            prs.push(mv);
                        }
                        break; // highest-level resource is prerequisite enough
                    }
                }
            }
            if let Upgrade::Producer(prod) = upg {
                if prod.prod_names.0 != "" {
                    pr.entry(Move::Switch(Switch {
                        uidx: iupg,
                        iprod: 1,
                    }))
                    .or_insert_with(Vec::new)
                    .push(Move::LvlUp(LvlUp {
                        uidx: iupg,
                        level: 1,
                    }));
                    for iprod in 0..2 {
                        pr.entry(Move::Switch(Switch { uidx: iupg, iprod }))
                            .or_insert_with(Vec::new)
                            .push(Move::Switch(Switch {
                                uidx: iupg,
                                iprod: (iprod + 1) % 2,
                            }));
                    }
                }
            }
        }
        self.prereqs = pr;
    }
}

impl Upgrade {
    pub fn get_name(&self) -> &str {
        match self {
            Upgrade::Producer(p) => &p.name[..],
            Upgrade::Boost(b) => &b.name[..],
        }
    }

    pub fn costs(&self) -> &Vec<Vec<i32>> {
        match self {
            Upgrade::Producer(p) => &p.costs,
            Upgrade::Boost(b) => &b.costs,
        }
    }
}

impl Producer {
    pub fn new(name: String) -> Producer {
        Producer {
            name,
            produces: Vec::new(),
            produces2: Vec::new(),
            spawn_time: 0.0,
            costs: Vec::new(),
            points: Vec::new(),
            points2: Vec::new(),
            needs: None,
            prod_names: ("".to_string(), "".to_string()),
        }
    }

    pub fn get_pt_rate(&self, level: usize, prod2: bool) -> f64 {
        if level < 1 {
            0.
        } else if prod2 {
            self.points2[level - 1] / self.spawn_time
        } else {
            self.points[level - 1] / self.spawn_time
        }
    }
}

impl Boost {
    pub fn new(name: String) -> Boost {
        Boost {
            name,
            res_bonus: Vec::new(),
            pt_mult: Vec::new(),
            time_mod: Vec::new(),
            costs: Vec::new(),
        }
    }
}

impl std::fmt::Display for LvlUp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}->{}", self.uidx, self.level)
    }
}

impl Switch {
    pub fn to_string(&self, g: &Game) -> String {
        if let Upgrade::Producer(prod) = &g.upgrades[self.uidx] {
            let pname = if self.iprod == 0 {
                &prod.prod_names.0
            } else {
                &prod.prod_names.1
            };
            format!("{}{}", self.uidx, pname)
        } else {
            "".to_string()
        }
    }
}

impl Move {
    pub fn to_string(&self, g: &Game) -> String {
        match self {
            Move::LvlUp(lup) => format!("{}", lup.uidx),
            Move::Switch(sw) => format!("{}", sw.to_string(g)),
        }
    }
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Move::LvlUp(lvlup) => lvlup.fmt(f),
            Move::Switch(sw) => write!(f, "{}~{}", sw.uidx, sw.iprod),
        }
    }
}
