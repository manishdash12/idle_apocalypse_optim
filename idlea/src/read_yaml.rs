use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::{Deserialize};
use regex::Regex;

use crate::game_state::GameState;

#[derive(Debug, Deserialize)]
pub struct InitialConfig {
    pub game: String,
    gem_boost: i32,
    time_left: String,
    // #[serduse regex::Regex;e(default = 0.0)]
    // time_left_secs: f64,
    points: f64,
    resources: HashMap<String, f64>,
    levels: HashMap<String, usize>,
}

pub fn load_config_yaml<P: AsRef<Path>>(config_file: P) -> Result<InitialConfig, Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::open(config_file)?;
    let reader = BufReader::new(file);

    let config = serde_yaml::from_reader(reader)?;

    Ok(config)
}

impl InitialConfig {
    pub fn fix_state(&self, gs: &mut GameState) {
        gs.gem_boost = self.gem_boost;
        gs.points = self.points;
        gs.time = time_left_to_time(&self.time_left, gs.g.event_time);

        let mut res_idx: HashMap<String, usize> = HashMap::new();
        for (ires, res_name) in gs.g.res_names.iter().enumerate() {
            res_idx.insert(res_name.to_string().to_lowercase(), ires);
        }
        for (res_name, amt) in self.resources.iter() {
            gs.res_amt[res_idx[&res_name.to_lowercase()]] = *amt;
        }

        let mut upg_idx: HashMap<String, usize> = HashMap::new();
        for (iupg, upg) in gs.g.upgrades.iter().enumerate() {
            upg_idx.insert(upg.get_name().to_string().to_lowercase(), iupg);
        }
        for (upg_name, lvl) in self.levels.iter() {
            gs.levels[upg_idx[&upg_name.to_lowercase()]] = *lvl;
        }
    }
}

fn time_left_to_time(time_left: &str, event_time: f64) -> f64 {
    // time_left is a string like: "1d 12h 7m 12s"
    let mut t: u32 = 0;
    let time_left_re = Regex::new(
        r"(?ix)
        ^\s*((?P<d>\d+)d)?
        \s*((?P<h>\d+)h)?
        \s*((?P<m>\d+)m)?
        \s*((?P<s>\d+)s)?
        \s*$",
    )
    .unwrap();
    let caps = time_left_re.captures(time_left).unwrap();
    if let Some(days) = caps.name("d") {
        t += 24 * 60 * 60 * days.as_str().parse::<u32>().unwrap();
    }
    if let Some(hours) = caps.name("h") {
        t += 60 * 60 * hours.as_str().parse::<u32>().unwrap();
    }
    if let Some(mins) = caps.name("m") {
        t += 60 * mins.as_str().parse::<u32>().unwrap();
    }
    if let Some(secs) = caps.name("s") {
        t += secs.as_str().parse::<u32>().unwrap();
    }
    let t = t as f64;
    assert!(
        t < event_time,
        "Time remaining {} exceeds event time {}",
        t,
        event_time
    );
    event_time - t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_left() {
        let event_time = (3 * 24 * 60 * 60) as f64;
        let tl = time_left_to_time("1d 2h 3m 4s", event_time);
        assert_eq!(tl, event_time - ((((1 * 24) + 2) * 60 + 3) * 60 + 4) as f64);
    }
}
