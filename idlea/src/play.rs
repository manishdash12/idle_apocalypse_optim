use std::cmp::Ordering;
use std::io;
use std::collections::HashMap;

use crate::game::Game;
use crate::game_state::GameState;

pub fn play(g: &Game, gs: &mut GameState) {
    gs.update_rates();
    'moves: loop {
        gs.print_status();
        gs.print_levels();

        let mut options = Vec::new();
        for iupg in 0..g.upgrades.len() {
            let ttl = gs.time_till_lvlup(iupg);
            if let Some(ttl) = ttl {
                options.push((iupg as usize, ttl));
            }
        }
        if options.len() == 0 {
            println!("Event Finished");
            let t_remaining = g.event_time - gs.time;
            gs.advance_time(t_remaining);
            gs.print_status();
            break;
        }

        options
            .sort_by(|(_, ttl_a), (_, ttl_b)| ttl_a.partial_cmp(ttl_b).unwrap_or(Ordering::Equal));

        let mut valid_ch = HashMap::new();
        for (iupg, ttl) in &options {
            println!(
                "{:2}: {} in {:.2} min, costs {:?}",
                iupg,
                g.upgrades[*iupg].get_name(),
                *ttl / 60.0,
                g.upgrades[*iupg].costs()[gs.levels[*iupg]]
            );
            valid_ch.insert(iupg.to_string(), (*iupg, *ttl));
        }
        let (iupg, ttl) = loop {
            print!("Enter choice (ex to exit): ");
            let mut choice = String::new();
            let bytes = io::stdin()
                .read_line(&mut choice)
                .expect("Failed to read line");
            if bytes == 0 {
                break 'moves;
            }
            if choice.to_lowercase().contains("ex") {
                break 'moves;
            }
            let choice_trim = choice.trim();
            match valid_ch.get(choice_trim) {
                Some(ch) => {
                    break ch;
                }
                None => {
                    continue;
                }
            }
        };
        let iupg = *iupg;
        let ttl = ttl.ceil() + 1.0; // extra padding for "safety"

        println!(
            "advancing {} seconds to upgrade {} -> {}",
            ttl,
            g.upgrades[iupg].get_name(),
            gs.levels[iupg] + 1
        );
        gs.advance_time(ttl);
        gs.level_up(iupg);
        println!("");
    }
}
