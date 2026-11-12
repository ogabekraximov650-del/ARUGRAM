#!/usr/bin/env python3
"""UZ: worker-rs/src/*.rs dagi statik CREATE/ALTER buyruqlaridan `schema.sql` yasaydi (jurnal-baza boshlang'ich sxemasi). Tartib: jadvallar, ALTER, indekslar.
EN: builds schema.sql (bootstrap schema of the journal store) from the static CREATE/ALTER strings in worker-rs/src. Order: tables, ALTERs, indexes."""
import glob, re, os
here = os.path.dirname(os.path.abspath(__file__))
rx = re.compile(r'(?:\(\s*|=\s*|\bexec\(\s*\w+\s*,\s*)"((?:[^"\\]|\\.)*)"\s*(?=,\s*vec!\[|;|,\s*&)', re.S)
tables, alters, indexes, seen = [], [], [], set()
for f in sorted(glob.glob(os.path.join(here, '..', '..', 'worker-rs', 'src', '*.rs'))):
    for m in rx.finditer(open(f).read()):
        t = m.group(1).replace('\\\n', '').replace('\\n', '\n').replace('\\"', '"').replace('\\\\', '\\')
        t = re.sub(r'\\\s*\n\s*', ' ', t)
        t = re.sub(r'--[^\n]*', '', t).strip()
        if '{' in t or t in seen: continue
        if re.match(r'CREATE\s+TABLE\s+IF\s+NOT\s+EXISTS\s+(?!_)\w+', t, re.I) and 'sqlmem' not in f and 'kvsql' not in f: tables.append(t)
        elif re.match(r'ALTER\s+TABLE\s+\w+\s+ADD\s+COLUMN', t, re.I): alters.append(t)
        elif re.match(r'CREATE\s+(UNIQUE\s+)?INDEX\s+IF\s+NOT\s+EXISTS', t, re.I) and 'sqlmem' not in f and 'kvsql' not in f: indexes.append(t)
        else: continue
        seen.add(t)
out = '\n'.join(s.rstrip(';') + ';\n' for s in tables + alters + indexes)
open(os.path.join(here, 'schema.sql'), 'w').write(out)
print(len(tables), 'jadval,', len(alters), 'alter,', len(indexes), 'indeks')
