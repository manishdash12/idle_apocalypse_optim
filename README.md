# idle_apoc_abridged

Some programs for simulating Idle Apocalypse events and optimizing gameplay.

This is not high-quality code, just a kind of personal playground. There are bugs,
most of which I'm not yet aware of.

## Python version
Play the Other tower event, "by hand". This will record your game in the file `mygame.csv`.

 ```
 ./play -g g/other_tower.csv -b 1
 ```

Find improvements:

```
./improve -g g/other_tower.csv -b 1 mygame.csv
```

That should do some optimization and produce `best_moves.txt`. Please keep these files
to yourself, only for your own games.

There are other command-line options. Try `-h` for a summary.

This uses arrays from `numpy`, which it turns out only slows things down. This could be made
more efficient in many ways including moving to native lists and running with PyPy,
but since a Rust-based optimizer works now... If you want
speed, just use/develop that.

## Rust version

This was created as a kind of self-teaching project to learn Rust better. So the code is in
no way exemplary of good practices.

```
cargo run --release -- play -g g/other_tower.csv -b 1
```

This is less-sophisticated than the Python version, and does NOT produce the replay, `mygame.csv`.

Find improvements:

```
cargo run --release -- imp -g g/other_tower.csv -b 1 -o best_moves.txt mygame.csv
```

This runs about 1000x faster than the Python program, and with various options can run
multi-threaded. It produces a output as a .txt file, which can be edited and given as input to both
improvement and play programs (both Python and Rust versions). For example:

```
./play -g g/other_tower.csv -b 1 < best_moves.txt
```

Also note the "rand" sub-command for the Rust program, which does awesome things. Also both
versions have the ability to play from a starting point, maybe mid-game, by reading a YAML
config file with the `-c` option.

## TO-DO

* Better handle games ending with production switches, maybe including an `end` command
  for `./play`.

* Add a `dump` command to `./play` to produce a YAML file mid-game. It may be that early
  move sequences are more definitive than mid- and end-game sequences, and that optimizing
  from a known-good starting point will produce better strategies faster.

* Include production switches in YAML files

* Try more ways to generate better random replays, maybe drawing from moves using
  weights from a neural net.

  * Extend/override the game time when generating random sequences
