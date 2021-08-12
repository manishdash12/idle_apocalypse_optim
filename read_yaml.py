import re
import yaml

import read_csv


def game_from_yaml(config_file):
    with open(config_file) as fin:
        config = yaml.load(fin)
    g = read_csv.game_from_csv(config['game'])
    g.set_gem_boost(config['gem_boost'])
    g.time = g.event_time - time_left_to_sec(config['time_left'])
    g.points = config['points']
    for res, amt in config['resources'].items():
        g.res_amt[g.res_name_idx[res]] = amt
    for upg, lvl in config['levels'].items():
        g.upgrades[g.upgrade_idx[upg]].level = lvl
    return g


def time_left_to_sec(time_left):
    time_left_re = re.compile(
        r"""(?ix)
        ^\s*((?P<d>\d+)d)?
        \s*((?P<h>\d+)h)?
        \s*((?P<m>\d+)m)?
        \s*((?P<s>\d+)s)?
        \s*$"""
    )
    m = time_left_re.match(time_left)
    assert m, f"time_left={time_left!r} is not valid"
    t = int(m.group('s')) if m.group('s') else 0
    t += int(m.group('m'))*60 if m.group('m') else 0
    t += int(m.group('h'))*60*60 if m.group('h') else 0
    t += int(m.group('d'))*60*60*24 if m.group('d') else 0
    return t
