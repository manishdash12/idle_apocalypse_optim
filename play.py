from game import NEVER
import math
import csv
from datetime import timedelta


def play(g, rec=None):
    g.update_rates()
    history = []
    while True:
        g.print_status()
        g.print_levels()
        # g.print_rates()
        options = []
        for i in range(len(g.upgrades)):
            ttl = g.time_till_lvlup(i)
            if ttl != NEVER:
                options.append([i, ttl])
        if not options:
            print('Event finished')
            t_remaining = g.event_time - g.time
            g.advance_time(t_remaining)
            g.print_status()
            rec.made_move(-1)
            break

        options.sort(key=lambda x: x[1])  # sort by time-to-level

        # Include fractional rate increases
        for opt in options:
            opt.append(' '.join([f'{x:.1f}%' for x in g.pcnt_boost(opt[0])]))

        while True:
            print('Upgrade choices:')
            valid = {}
            for opt in options:
                (i, ttl, boost) = opt
                valid[str(i)] = opt
                upg = g.upgrades[i]
                cost = upg.costs[upg.level]
                print(f'{i:2d}: {upg.name:{g.upg_name_len}s} in {ttl/60:.2f} min,',
                      f'costs {cost} boosts: {boost}')
                delay_news = []
                accel_news = []
                for aopt in options:
                    (ai, attl, _) = aopt
                    if i == ai:
                        continue
                    tau = g.upg_time_after_upg(i, ttl, ai)
                    if tau - attl > 1:
                        shift_min = (tau - attl) / 60
                        delay_news.append(f'{ai} + {shift_min:.1f}')
                    elif attl - tau > 1:
                        shift_min = (attl - tau) / 60
                        accel_news.append(f'{ai} - {shift_min:.1f}')
                if accel_news:
                    print('      accelerates:', ',  '.join(accel_news), 'minutes')
                if delay_news:
                    print('      delays:', ',  '.join(delay_news), 'minutes')
            chp = {}
            if g.chprod_choices:
                for chs, (up, i) in g.chprod_choices.items():
                    if up.level == 0:
                        continue
                    if id(up.produces) == id(up.prod1) and i == 1:
                        continue
                    if id(up.produces) == id(up.prod2) and i == 2:
                        continue
                    chp[chs] = (up, i)
                if chp:
                    print('Production switches:', ', '.join(sorted(chp.keys())))
            ch = input('Enter choice: ').strip()
            ch = ch.lower()
            if ch in valid or ch in chp:
                break
            if ch == 'u' and history:
                break
            print('Try another choice')
        if ch == 'u':
            print('undoing previous choice')
            g = history.pop(-1)
            if rec:
                rec.del_move(g)
            continue
        history.append(g.copy())
        if ch in valid:
            (ich, ttl, _) = valid[ch]
            ttl = math.ceil(ttl) + 1  # just being safe
            print(f'advancing {ttl} seconds to upgrade {g.upgrades[ich].name} ->'
                  f' {g.upgrades[ich].level+1}')
            g.advance_time(ttl)
            g.level_up(ich)
            if rec:
                rec.made_move(ich, [o[0] for o in options])
        else:
            (up, i) = g.chprod_choices[ch]
            print(f'Switching production on {up.name} to {i}')
            g.change_prod(ch)
            if rec:
                rec.made_move(ch)
        print()
    if rec:
        rec.done()


class Recorder:
    def __init__(self, csv_file, game):
        self.csv_file = csv_file
        self.g = game
        self.fout = open(csv_file, 'w')
        self.rec = csv.writer(self.fout)
        self.time = game.time
        header = ['time left', 'after (min)', 'upg #', 'upgrade', 'cost']
        for res in self.g.res_names:
            header.append(f'{res[:2]}/min')
        for upg in self.g.upgrades:
            header.append(upg.name)
        header.append(self.g.points_name)
        self.rec.writerow(header)
        self.rows = []

    def made_move(self, upg_idx, choices=[]):
        after = self.g.time - self.time
        self.time = self.g.time
        time_left = str(timedelta(seconds=self.g.event_time - self.g.time))
        if isinstance(upg_idx, str):
            (up, _) = self.g.chprod_choices[upg_idx]
            prod_letter = upg_idx[-1]
            upg_txt = f"{up.name}->{prod_letter}"
            cost_txt = ''
        else:
            upg = self.g.upgrades[upg_idx]
            upg_txt = f'{upg.name} -> {upg.level}'
            cost_txt = []
            for amt, n in zip(upg.costs[upg.level-1], self.g.res_names):
                if amt > 0:
                    cost_txt.append(f'{short(amt)} {n[:2]}')
            cost_txt = ', '.join(cost_txt)
        row = [time_left, f'{after/60:.1f}', upg_idx, upg_txt, cost_txt]
        for rt in self.g.res_rate:
            row.append(f'{rt*60:.3f}')
        for i in range(len(self.g.upgrades)):
            if i == upg_idx:
                row.append(self.g.upgrades[i].level)
            elif i in choices and upg_idx in choices and choices.index(upg_idx) > choices.index(i):
                row.append('-')  # deferred choice
            else:
                row.append('')
        row.append(short(self.g.points, '.3f'))
        self.rows.append(row)

    def del_move(self, new_game):
        self.rows.pop(-1)
        self.g = new_game

    def done(self):
        self.rec.writerows(self.rows)
        self.fout.close()


def short(amount, prec=None):
    postfix = ''
    for pf in ['k', 'M', 'B', 'T']:
        if amount > 1000:
            amount /= 1000
            postfix = pf
        else:
            break
    if prec is None:
        return f'{amount}{postfix}'
    else:
        return f'{amount:{prec}}{postfix}'
