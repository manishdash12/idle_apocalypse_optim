import csv
from locale import atoi

from game import Game, Producer, Boost


def game_from_csv(csv_file):
    g = Game()
    with open(csv_file) as fin:
        cr = csv.reader(fin)

        r = next(cr)
        g.name = r[0]
        if r[6]:
            g.goal = atoi(r[6])
            print("goal:", g.goal)

        next(cr)
        r = next(cr)
        res_names = []
        for rname in r[2:]:
            if not rname:
                g.points_name = res_names.pop(-1)
                break
            res_names.append(rname)
        g.set_resources(res_names)

        next(cr)
        upg = None
        for r in cr:
            if upg is None and r[0]:
                upname = r[0]
                if 'boost' in upname.lower() or 'speed' in upname.lower():
                    print(f'Loading boost: {upname}')
                    upg = Boost(upname)
                else:
                    print(f'Loading producer: {upname}')
                    upg = Producer(upname)
                    if r[2] and r[13]:
                        upg.prod_names = (r[2], r[13])
            elif r[0] == '':
                # blank line (1st column) separates rows
                # print('finished', upname)
                if isinstance(upg, Producer) and len(g.upgrades) > 0:
                    upg.needs = len(g.upgrades) - 1  # each successive producer needs the previous
                g.add_upgrade(upg)
                upg = None
            else:
                upg.new_level(r, g.nres)

        if upg:
            # print('finished', upname)
            g.add_upgrade(upg)
    return g
