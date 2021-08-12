import locale
from locale import atoi, atof
import numpy as np
import math
from datetime import timedelta
import copy

locale.setlocale(locale.LC_ALL, locale.getlocale())

NEVER = None


class Game:
    def __init__(self):
        self.name = "event"
        self.res_names = []
        self.res_name_idx = {}
        self.upgrades = []  # instances of Producer or Boost
        self.upgrade_idx = {}
        self.res_amt = []
        self.res_rate = []
        self.pt_rate = 0
        self.time = 0
        self.event_time = 3 * 24 * 60 * 60
        self.goal = 0
        self.points_name = "points"  # "damage", etc
        self.points = 0
        self.nres = 0
        self.bonuses = []
        self.gem_boost = []
        self.commercial_mod = -0.25  # times increased by this (negative) factor
        self.ttl_overshoot = 20  # seconds beyond when upgrade is available
        self.res_name_len = 20  # after set_resources, the longest resource length
        self.upg_name_len = 20  # similar
        self.chprod_choices = {}
        self.chprod_complements = {}

    def time_left(self):
        return max(self.event_time - self.time, 0)

    def add_upgrade(self, up):
        self.upgrades.append(up)
        self.upgrade_idx[up.name] = len(self.upgrades) - 1
        if len(self.upgrades) == 1:
            self.upg_name_len = len(up.name)
        elif len(up.name) > self.upg_name_len:
            self.upg_name_len = len(up.name)
        for cost in up.costs[0]:
            if cost > 0:
                break
        else:
            up.level = 1
        if hasattr(up, "prod2") and up.prod2 is not None:
            upg_idx = len(self.upgrades)-1
            self.chprod_choices[f"{upg_idx}{up.prod_names[0]}"] = (up, 1)
            self.chprod_choices[f"{upg_idx}{up.prod_names[1]}"] = (up, 2)
            self.chprod_complements[f"{upg_idx}{up.prod_names[0]}"] = f"{upg_idx}{up.prod_names[1]}"
            self.chprod_complements[f"{upg_idx}{up.prod_names[1]}"] = f"{upg_idx}{up.prod_names[0]}"

    def set_resources(self, res_names):
        self.res_names = res_names
        self.nres = len(res_names)
        self.res_amt = np.zeros([self.nres], dtype=float)
        self.res_rate = np.zeros([self.nres], dtype=float)
        self.bonuses = np.zeros([self.nres], dtype=int)
        self.gem_boost = np.zeros([self.nres], dtype=int)
        self.res_name_len = max([len(n) for n in res_names])
        for i, res_name in enumerate(self.res_names):
            self.res_name_idx[res_name] = i

    def set_gem_boost(self, bonus):
        self.gem_boost += int(bonus)

    def update_rates(self):
        self.res_rate.fill(0)
        self.bonuses.fill(0)
        pt_mult = 1.0
        time_fact = 1.0
        for upg in filter(lambda x: isinstance(x, Boost), self.upgrades):
            upg.add_bonuses(self.bonuses)
            time_fact = upg.add_time_mod(time_fact)
            pt_mult = upg.get_pt_mult(pt_mult)
        self.pt_rate = 0
        self.bonuses += self.gem_boost
        for upg in filter(lambda x: isinstance(x, Producer), self.upgrades):
            upg.add_rates(self.res_rate, self.bonuses)
            self.pt_rate += upg.get_pt_rate()
            # print(f'    pt_rate: {upg.name} -> {self.pt_rate}')
        self.pt_rate *= pt_mult
        time_fact += self.commercial_mod
        # print(f'    pt_mult={pt_mult}')
        self.pt_rate /= time_fact
        self.res_rate /= time_fact
        # print('rates:', repr(self.res_rate), repr(self.pt_rate))

    def print_rates(self):
        print("Rates:")
        for i, res in enumerate(self.res_names):
            rpm = self.res_rate[i] * 60
            print(f'   {res} at {rpm:.2f} per min')
        ppm = self.pt_rate * 60
        print(f'   {self.points_name} at {ppm:.3f} per min')

    def advance_time(self, dt):
        self.time += dt
        if self.time > self.event_time:
            dt -= self.time - self.event_time
            self.time = self.event_time
        self.points += dt * self.pt_rate
        self.res_amt += dt * self.res_rate

    def print_status(self):
        print(f'Status at t={timedelta(seconds=self.time)}')
        for i, res in enumerate(self.res_names):
            rpm = self.res_rate[i] * 60
            print(f'   {res:{self.res_name_len}s}: {_lfmt(self.res_amt[i])}  {rpm:.2f}/min')
        ppm = self.pt_rate * 60
        print(f'   {self.points_name:{self.res_name_len}s}: {_lfmt(self.points)}  {ppm:.0f}/min')

    def print_levels(self):
        print('Upgrades:')
        for upg in self.upgrades:
            if upg.level > 0:
                print(f'   {upg.name:{self.upg_name_len}s}: {upg.level}')

    def level_up(self, upgrade):
        if isinstance(upgrade, str):
            upg = self.upgrade_idx[upgrade]
        else:
            upg = self.upgrades[upgrade]
        upg.level += 1
        if upg.level > len(upg.costs):
            raise ValueError(f'Upgrade {upg.name} is at max level already')
        cost = upg.costs[upg.level-1]
        # if not (cost <= self.res_amt).all():
        #     raise ValueError(f'Cost {cost} exceeds resources on hand: {self.res_amt}')
        self.res_amt -= cost
        self.update_rates()

    def time_till_lvlup(self, upgrade):
        if isinstance(upgrade, str):
            upg = self.upgrade_idx[upgrade]
        else:
            upg = self.upgrades[upgrade]
        nxt_lvl = upg.level + 1
        if nxt_lvl > len(upg.costs):
            return NEVER
        if hasattr(upg, 'needs') and upg.needs is not None and self.upgrades[upg.needs].level == 0:
            return NEVER  # unmet prereq
        cost = upg.costs[nxt_lvl - 1]
        if (cost <= self.res_amt).all():
            # already have enough
            return 0
        need = cost - self.res_amt
        need = np.vectorize(lambda x: math.ceil(x) if x > 0 else 0.0)(need)
        max_t = 0
        for i, nr in enumerate(need):
            if nr > 0:
                if self.res_rate[i] > 0:
                    t = nr / self.res_rate[i]
                    max_t = max(t, max_t)
                    # print(f'      {upg.name} res {i} nr={nr} rt={self.res_rate[i]} t={t}')
                else:
                    return NEVER
        if max_t > self.event_time - self.time:
            return NEVER
        return max_t + self.ttl_overshoot

    def pcnt_boost(self, upgrade):
        # How much *would* this upgrade increase rates?
        rates = np.zeros([self.nres + 1], dtype=float)
        self.upgrades[upgrade].level += 1
        self.update_rates()
        np.copyto(rates[:self.nres], self.res_rate)
        rates[self.nres] = self.pt_rate
        self.upgrades[upgrade].level -= 1
        self.update_rates()
        rates[:self.nres] /= self.res_rate
        rates[self.nres] /= self.pt_rate
        rates = (rates - 1) * 100
        return rates

    def upg_time_after_upg(self, first_uidx, after, second_uidx):
        "Returns the time from now till the second upgrade can be done"
        alt_self = self.copy()
        alt_self.advance_time(after)
        alt_self.level_up(first_uidx)
        new_time = alt_self.time_till_lvlup(second_uidx)
        if new_time == NEVER:
            return 999999
        return new_time + after

    def copy(self):
        return copy.deepcopy(self)
        # gc = copy.copy(self)
        # gc.upgrades = copy.deepcopy(self.upgrades)
        # ...

    def finish(self):
        "Run out the remaining time, if any"
        t_remaining = self.event_time - self.time
        if t_remaining > 0:
            self.advance_time(t_remaining)

    def change_prod(self, ch):
        (up, i) = self.chprod_choices[ch]
        if i == 1:
            up.produces = up.prod1
            up.points = up.points1
        else:
            up.produces = up.prod2
            up.points = up.points2
        self.update_rates()



class Producer:
    def __init__(self, name):
        self.name = name
        self.level = 0
        self.produces = []  # [level][resource] = addtn'l per spawn
        self.prod2 = None
        self.prod1 = None
        self.spawn_time = 0
        self.costs = []  # array of arrays of costs: [level][resource]
        self.points = []  # [level] = points
        self.points1 = None
        self.points2 = None
        self.needs = None
        self.prod_names = None

    def new_level(self, row, nres):
        if not self.spawn_time:
            self.spawn_time = atoi(row[1])
        self.produces.append(_as_ints(row[2:2+nres]))
        self.points.append(atoi(row[2+nres]) if row[2+nres] else 0)
        self.costs.append(_as_ints(row[4+nres:4+nres*2]))
        if len(row) >= 6 + 3 * nres:
            prod2 = _as_ints(row[5+nres*2:6+nres*3])
            # print(f'{self.name} prod2 appending {prod2}')
            if len(self.produces) == 1:  # first row, decide yay or nay on 2nd production
                yay = False
                for amt in prod2:
                    if amt > 0:
                        yay = True
                        break
                if not yay:
                    # maybe we have a pause choice
                    for amt in self.produces[0]:
                        if amt < 0:
                            # this one has a cost we might not want to pay
                            yay = True
                            break
                yay = yay or self.prod_names
                if yay:
                    self.prod2 = []
                    self.prod1 = self.produces
                    self.points2 = []
                    self.points1 = self.points
                    if not self.prod_names:
                        self.prod_names = ('a', 'z')
            if self.prod2 is not None:
                self.prod2.append(prod2[:-1])
                self.points2.append(prod2[-1])

    def add_rates(self, rates, bonuses):
        if self.level < 1:
            return
        for i, prod in enumerate(self.produces[self.level-1]):
            bonus = bonuses[i] if prod > 0 else 0  # bonuses only apply to production
            # print(f'{rates[i]!r}, {prod!r}, {bonus!r}, {self.spawn_time!r}')
            rates[i] += (prod + bonus) / self.spawn_time

    def get_pt_rate(self):
        if self.level < 1:
            return 0
        return self.points[self.level-1] / self.spawn_time

    def __deepcopy__(self, memo):
        # Only return a shallow copy of yourself -- only level can change
        cp = copy.copy(self)
        return cp


class Boost:
    def __init__(self, name):
        self.name = name
        self.level = 0
        self.res_bonus = []
        self.pt_mult = []  # > 1 is good
        self.time_mod = []  # < 1 is better
        self.costs = []

    def new_level(self, row, nres):
        self.time_mod.append(atof(row[1]) if row[1] else 0.0)
        self.res_bonus.append(_as_floats(row[2:2+nres]))
        self.pt_mult.append(1.0 + atof(row[2+nres]) if row[2+nres] else 1.0)
        self.costs.append(_as_ints(row[4+nres:4+nres*2]))

    def add_bonuses(self, bonuses):
        if self.level < 1:
            return
        for i, bonus in enumerate(self.res_bonus[self.level-1]):
            bonuses[i] += bonus

    def get_pt_mult(self, pts):
        if self.level < 1:
            return pts
        return pts * self.pt_mult[self.level-1]

    def add_time_mod(self, tm):
        if self.level < 1:
            return tm
        return tm + self.time_mod[self.level-1]

    def __deepcopy__(self, memo):
        # Only return a shallow copy of yourself -- only level can change
        return copy.copy(self)


def _as_ints(values):
    return [atoi(x) if x else 0 for x in values]


def _as_floats(values):
    return [atoi(x) if x else 0.0 for x in values]


def _lfmt(value: float) -> str:
    return locale.format_string('%.0f', value, grouping=True)
