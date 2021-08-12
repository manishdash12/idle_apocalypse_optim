import collections
import csv
import math
import sys

import game


Upgrade = collections.namedtuple('Upgrade', 'upg_idx level')

upg_prereq = collections.defaultdict(list)  # those upgrades that are prerequisite to the key


def init_prereqs(g):
    first_producer = [None for _ in range(g.nres)]
    for upg_idx, upg in enumerate(g.upgrades):
        if not isinstance(upg, game.Producer):
            continue
        for ires, amt in enumerate(upg.produces[0]):
            if amt > 0 and first_producer[ires] is None:
                first_producer[ires] = Upgrade(upg_idx, 1)
    # print('First producers:')
    # for ires, upg in enumerate(first_producer):
    #     print(ires, upg)
    for upg_idx, upg in enumerate(g.upgrades):
        for ilvl in range(len(upg.costs)):
            if ilvl == 0:
                # Producers must be listed first for this to work
                # To relax this requirement I'd need to build a list of producers
                if isinstance(upg, game.Producer) and upg_idx > 0:
                    # prereq is level 1 of the previous upgrade
                    upg_prereq[Upgrade(upg_idx, 1)].append(Upgrade(upg_idx-1, 1))
            else:
                # Previous level needs to be unlocked
                upg_prereq[Upgrade(upg_idx, ilvl+1)].append(Upgrade(upg_idx, ilvl))
            # assume upgrades are in order of resource index produced
            for ires in range(g.nres-1, -1, -1):
                if upg.costs[ilvl][ires] > 0:
                    upg_prereq[Upgrade(upg_idx, ilvl+1)].append(first_producer[ires])
                    break  # highest-level resource is prerequisite enough
    for chp, (up, i) in g.chprod_choices.items():
        upg_idx = g.upgrades.index(up)
        assert upg_idx >= 0
        upg_prereq[chp].append(Upgrade(upg_idx, 1))
        if chp in g.chprod_complements:
            upg_prereq[chp].append(g.chprod_complements[chp])
    # print('Pre-requisites:')
    # for u, pre in upg_prereq.items():
    #     print(u, pre)
    # TODO: prune these upgrades


def load_sequence(csvfile, g):
    "Returns a list of Upgrades"
    # levels = collections.defaultdict(int)  # indexed by upgrade index, holds upgrade level
    levels = [upg.level for upg in g.upgrades]
    upgrades = []

    def add_upgrade(upg_idx):
        if upg_idx in g.chprod_choices:
            upgrades.append(upg_idx)
            return
        upg_idx = int(upg_idx)
        levels[upg_idx] += 1
        upgrades.append(Upgrade(upg_idx, levels[upg_idx]))

    with open(csvfile) as fin:
        rows = csv.reader(fin)
        h = next(rows)
        if len(h) == 1 and _is_an_int(h[0]):
            # it's a simple file with numbers
            upg_idx_col = 0
            add_upgrade(h[0])
        else:
            upg_idx_col = h.index('upg #')
        for row in rows:
            add_upgrade(row[upg_idx_col])
    if upg_idx_col > 0:
        upgrades = upgrades[:-1]  # last row is not an upgrade
    return upgrades


def _is_an_int(numstr):
    try:
        int(numstr)
        return True
    except ValueError:
        return False


def score(seq, g):
    "returns a game score using this sequence"
    g = g.copy()
    g.update_rates()
    for upg in seq:
        # print(f"{upg}: t={g.time}, pt_rate={g.pt_rate}, points={g.points}")
        if upg in g.chprod_choices:
            g.change_prod(upg)
            # print("did", upg)
            # g.print_status()
            # g.print_levels()
            continue
        ttl = g.time_till_lvlup(upg.upg_idx)
        if ttl == game.NEVER:
            # print(f"Can't upgrade {g.upgrades[upg.upg_idx].name} -> {upg.level}")
            break
        ttl = math.ceil(ttl) + 1
        # print(f'Advancing {ttl/60:.2f} minutes')
        g.advance_time(ttl)
        # print(f'Upgrading {g.upgrades[upg.upg_idx].name} -> {upg.level}')
        g.level_up(upg.upg_idx)
        # g.print_status()
    g.finish()
    return g.points


def score_spare(seq, g):
    "Returns the number of hours remaining after winning"
    g = g.copy()
    g.update_rates()
    for upg in seq:
        if upg in g.chprod_choices:
            g.change_prod(upg)
            continue
        ttl = g.time_till_lvlup(upg.upg_idx)
        if ttl == game.NEVER:
            break
        ttl = math.ceil(ttl) + 1
        if g.points + g.pt_rate * ttl > g.goal and g.pt_rate > 0:
            dt_win = (g.goal - g.points) / g.pt_rate
            spare = g.event_time - (g.time + dt_win)
            return spare / 60 / 60
        g.advance_time(ttl)
        g.level_up(upg.upg_idx)
    g.finish()
    if g.pt_rate == 0:
        spare = -1e30
    else:
        dt_win = (g.goal - g.points) / g.pt_rate
        spare = -dt_win
    return spare / 60 / 60


def find_improvement(seq, seq_score, g, pushy=False, second=False, spare=True):
    """
    Try all re-orderings of a single upgrade for the best score
    returns the best ordering and its score
    """
    if spare:
        my_score = score_spare
    else:
        my_score = score
    dots = True
    best_score = seq_score
    best_seq = seq.copy()
    if pushy:
        new_seqs = try_seqs_pushy(seq)
    else:
        new_seqs = try_seqs(seq)
    for new_seq in new_seqs:
        try:
            s = my_score(new_seq, g)
        except ValueError:
            continue
        # print(' score improves by', (s - best_score)/best_score * 100, '%')
        if s > best_score:
            best_score = s
            best_seq = new_seq.copy()
            if dots:
                print('!', end='')
        elif dots:
            print('.', end='')
        if second:
            for s2 in try_seqs(new_seq):  # not pushy
                try:
                    s = my_score(s2, g)
                except ValueError:
                    continue
                if s > best_score:
                    best_score = s
                    best_seq = s2.copy()
                    if dots:
                        print('#', end='')
                # elif dots:
                #     print(',', end='')
        if dots:
            sys.stdout.flush()
    print()
    return (best_seq, best_score)


def try_seqs(seq):
    """
    Yields variations of the given upgrade sequence
    """
    for iup in range(1, len(seq)):  # could start at iup=0 too, assuming first upgrade is obvious
        up = seq[iup]
        prereq = upg_prereq[up]
        # print(f're-sequencing {up.upg_idx}->{up.level} -- prereqs are {prereq}')
        orig = seq
        seq = orig.copy()
        # explore advancements
        for iprev in range(iup-1, 0, -1):
            if seq[iprev] in prereq:
                break
            seq[iprev+1] = seq[iprev]
            seq[iprev] = up
            # print(f'Advancing {up.upg_idx}->{up.level}', end='')
            yield seq
        seq = orig.copy()
        # explore postponements
        for inext in range(iup+1, len(seq)):
            if up in upg_prereq[seq[inext]]:
                break
            seq[inext-1] = seq[inext]
            seq[inext] = up
            # print(f'Delaying {up.upg_idx}->{up.level}', end='')
            yield seq
        seq = orig.copy()


def try_seqs_pushy(seq):
    """
    Yields variations of the given upgrade sequence
    """
    for iup in range(1, len(seq)):  # could start at iup=0 too, assuming first upgrade is obvious
        # print(f're-sequencing {up.upg_idx}->{up.level} -- prereqs are {prereq}')
        orig = seq
        seq = orig.copy()
        # explore advancements
        ups = [seq[iup]]
        prereq = set(upg_prereq[ups[0]])
        for iprev in range(iup-1, 0, -1):
            if seq[iprev] in prereq:
                ups.insert(0, seq[iprev])
                prereq.update(upg_prereq[seq[iprev]])
                continue
            seq[iprev+len(ups)] = seq[iprev]
            seq[iprev:iprev+len(ups)] = ups
            # print(f'Advancing {up.upg_idx}->{up.level}', end='')
            yield seq
        seq = orig.copy()
        ups = [seq[iup]]
        ups_set = set(ups)
        # explore postponements
        for inext in range(iup+1, len(seq)):
            if not ups_set.isdisjoint(upg_prereq[seq[inext]]):
                # seq[inext] has a prereq that includes ups
                ups.append(seq[inext])
                ups_set.add(seq[inext])
                continue
            seq[inext-len(ups)] = seq[inext]
            seq[inext-len(ups)+1:inext+1] = ups
            # print(f'Delaying {up.upg_idx}->{up.level}', end='')
            yield seq
        seq = orig.copy()
