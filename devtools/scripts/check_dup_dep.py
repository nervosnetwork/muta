import glob

import toml

m = {}

for p in glob.glob('**/Cargo.toml', recursive=1):
    with open(p) as f:
        c = toml.load(f)
        for e, v in c['dependencies'].items():
            if isinstance(v, dict):
                continue
            if e in m and m[e][1] != v:
                print(f'dup! {m[e][0]} {e}={m[e][1]} | {p} {e}={v}')
            else:
                m[e] = [p, v]
        if 'dev-dependencies' in c:
            for e, v in c['dev-dependencies'].items():
                if isinstance(v, dict):
                    continue
                if e in m and m[e][1] != v:
                    print(f'dup! {m[e][0]} {e}={m[e][1]} | {p} {e}={v}')
                else:
                    m[e] = [p, v]
