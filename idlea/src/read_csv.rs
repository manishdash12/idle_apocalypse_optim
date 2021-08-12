use std::error::Error;
use std::fs::File;
//use std::io::prelude::*;

use crate::game::{Boost, Game, Producer, Upgrade};

pub fn game_from_csv(csv_file: &str) -> Result<Game, Box<dyn Error>> {
    let file = File::open(csv_file)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(file);
    let mut game = Game::new();
    let mut recs = rdr.records();

    //let first = recs.next().ok_or(Err("No header row"))?; // nope, must use a generic error lib
    let row = recs.next().unwrap()?;
    //println!("{:?}", row);
    game.name = row[0].to_string();

    // goal points
    let mut next_is_target = false;
    for next_col in row.iter().skip(1) {
        if next_is_target {
            println!("Goal: {:?}", next_col);
            game.goal = local_str_as_i64(next_col) as f64;
            break;
        }
        if next_col == "goal:" {
            next_is_target = true;
        }
    }

    // resource names
    recs.next();
    let row = recs.next().unwrap()?;
    for name in row.iter().skip(2) {
        if name.len() == 0 {
            game.points_name = game.res_names.pop().unwrap();
            break;
        }
        game.res_names.push(name.to_owned());
    }
    game.nres = game.res_names.len();
    for rname in game.res_names.iter() {
        if rname.len() > game.res_name_len {
            game.res_name_len = rname.len();
        }
    }

    // upgrades
    recs.next();
    let mut upg: Option<Upgrade> = None;
    for row in recs {
        let upname = row.as_ref().unwrap()[0].to_string();
        if upname == "" {
            // Blank line separates rows. Level rows start with "1", "2", etc.
            if let Some(mut finished_upg) = upg.take() {
                if let Upgrade::Producer(prod) = &mut finished_upg {
                    if game.upgrades.len() > 0 {
                        prod.needs = Some(game.upgrades.len() - 1);
                    }
                }
                game.add_upgrade(finished_upg);
            } else {
                println!("Unexpected separator row");
            }
        } else {
            // Either a new upgrade or a new level, current upgrade
            if let None = upg {
                let up_lower = upname.to_lowercase();
                if up_lower.contains("boost") || up_lower.contains("speed") {
                    println!("Loading boost: {}", upname);
                    upg = Some(Upgrade::Boost(Boost::new(upname)));
                } else {
                    println!("Loading producer: {}", upname);
                    let mut prod = Producer::new(upname);
                    if &row.as_ref().unwrap()[2] != "" {
                        prod.prod_names.0 = row.as_ref().unwrap()[2].to_string();
                        prod.prod_names.1 = row.as_ref().unwrap()[2 * game.nres + 5].to_string();
                    }
                    upg = Some(Upgrade::Producer(prod));
                }
            } else {
                add_level(upg.as_mut().unwrap(), row.as_ref().unwrap(), game.nres);
            }
        }
    }
    if let Some(mut last_upg) = upg.take() {
        if let Upgrade::Producer(prod) = &mut last_upg {
            if game.upgrades.len() > 0 {
                prod.needs = Some(game.upgrades.len() - 1);
            }
        }
        game.add_upgrade(last_upg);
    }

    game.find_prereqs();
    Ok(game)
}

fn add_level(upg: &mut Upgrade, row: &csv::StringRecord, nres: usize) {
    let fields: Vec<&str> = row.iter().collect(); // makes the StringRecord sliceable
                                                  // println!("Row is: {:?}", fields);
    match upg {
        Upgrade::Producer(prod) => {
            if prod.spawn_time == 0.0 {
                prod.spawn_time = fields[1].parse().unwrap();
            }
            prod.produces.push(
                fields[2..2 + nres]
                    .iter()
                    .map(|s| local_str_as_i64(s) as i32)
                    .collect(),
            );
            // println!("{}, nres={}, produces {:?}", prod.name, nres, fields[2..2+nres].iter());
            prod.points.push(local_str_as_i64(fields[2 + nres]) as f64);
            prod.costs.push(
                fields[4 + nres..4 + nres * 2]
                    .iter()
                    .map(|s| local_str_as_i64(s) as i32)
                    .collect(),
            );
            if prod.produces.len() == 1 && prod.prod_names.0 == "" {
                // First row, decide if it has any negative productions. If so, we can
                // pause production as a feasable production switch.
                if prod.produces[0].iter().any(|&x| x < 0) {
                    prod.prod_names = ("a".to_string(), "z".to_string());
                }
            }
            if fields.len() > 5 + 3 * nres {
                // 2nd production
                // println!("Adding {:?} to {} produces2", fields[5+2*nres..5+3*nres].iter(), prod.name);
                prod.produces2.push(
                    fields[5 + 2 * nres..5 + 3 * nres]
                        .iter()
                        .map(|s| local_str_as_i64(s) as i32)
                        .collect(),
                );
                prod.points2
                    .push(local_str_as_i64(fields[5 + 3 * nres]) as f64);
            } else {
                prod.produces2.push(vec![0; nres]);
                prod.points2.push(0.0);
            }
        }
        Upgrade::Boost(boost) => {
            if let Ok(tm) = fields[1].parse() {
                boost.time_mod.push(tm);
            } else {
                boost.time_mod.push(0.0);
            }
            boost.res_bonus.push(
                fields[2..2 + nres]
                    .iter()
                    .map(|s| local_str_as_i64(s) as i32)
                    .collect(),
            );
            boost.pt_mult.push(fields[2 + nres].parse().unwrap_or(0.0));
            boost.costs.push(
                fields[4 + nres..4 + nres * 2]
                    .iter()
                    .map(|s| local_str_as_i64(s) as i32)
                    .collect(),
            );
        }
    }
}

fn local_str_as_i64(s: &str) -> i64 {
    if let Ok(val) = s.replace(",", "").parse() {
        val
    } else {
        0
    }
}
