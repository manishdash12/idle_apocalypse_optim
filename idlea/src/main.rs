use std::process;
use docopt::Docopt;
use serde::Deserialize;

use idlea::game_state::GameState;
use idlea::game::Game;
use idlea::play::play;
use idlea::read_csv;
use idlea::improve;
use idlea::read_yaml;

const USAGE: &'static str = "
Idle Apoc Event helper

Usage:
  idlea play [options]
  idlea imp [options] <initial>
  idlea rand [options]
  idlea (-h | --help)

Options:
  -h --help            Show this screen.
  -g --game=<file>     game .csv file [default: g/happy-time.csv].
  -c --config=<file>   initial config YAML file.
  -b --boost=<num>     gem boost level [default: 1].
  -o --output=<file>   output data file.
  -d --depth=<num>     looping depth for improvements [default: 1].
  -f --fast            Pick first optimization, not the best.
  -s --switches=<str>  Use N0,N1,N2,... production switches for optimization.
  -p --pushy           Whether or not to try pushy variations.
  --dthr=<num>         Depth threshold -- don't go deep below this score. [default: -10.0]
";

// TODO: make some options specific to commands, more subcommand help

#[derive(Debug, Deserialize)]
struct Args {
    flag_game: String,
    flag_config: String,
    flag_output: String,
    flag_boost: i32,
    flag_depth: usize,
    flag_fast: bool,
    flag_switches: String,
    flag_pushy: bool,
    flag_dthr: f64,
    arg_initial: String,
    cmd_play: bool,
    cmd_imp: bool,
    cmd_rand: bool,
}

static mut GAME: Option<Game> = None;

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let config = if &args.flag_config != "" {
        let config = read_yaml::load_config_yaml(&args.flag_config).unwrap_or_else(|err| {
            println!("Problem reading config: {}", err);
            process::exit(1);
        });
        Some(config)
    } else {
        None
    };

    let game_csv = if let Some(config) = &config {
        // args.flag_game = config.game.clone();
        &config.game
    } else {
        &args.flag_game
    };

    let g = unsafe {
        GAME = Some(read_csv::game_from_csv(game_csv).unwrap());
        GAME.as_ref().unwrap()
    };
    println!("Game {}: {}", game_csv, g.name);

    // println!("{:#?}", g);
    let mut gs = GameState::new_from_game(g);

    if let Some(config) = &config {
        config.fix_state(&mut gs);
    } else {
        gs.gem_boost = args.flag_boost;
    }
    println!("Boost = {}", gs.gem_boost);
    // println!("{:#?}", gs);

    unsafe {
        improve::set_pushy(args.flag_pushy);
    }

    if args.cmd_play {
        play(&g, &mut gs);
    } else if args.cmd_imp {
        improve::improve_main(
            gs,
            &args.arg_initial,
            &args.flag_output,
            args.flag_depth,
            args.flag_fast,
        );
    } else if args.cmd_rand {
        let switches = improve::switches_from_arg(&args.flag_switches);
        improve::improve_main_random(
            gs,
            &args.flag_output,
            args.flag_depth,
            args.flag_fast,
            &switches,
            args.flag_dthr,
        );
    }
}
