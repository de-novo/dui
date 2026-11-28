"""Compare CSS px box components for each fixture. Run next to the two CSVs."""

import csv
import json
from pathlib import Path


def read(name):
    return [
        list(map(float, row)) for row in csv.reader(Path(name).read_text().splitlines())
    ]


reference = {tuple(r[:6]): r[6:] for r in read("reference.csv")}
assert len(reference) == 288
groups = {}
seen = set()
for row in read("taffy.csv"):
    key = tuple(row[:6])
    mode = int(row[6])
    assert (key, mode) not in seen
    seen.add((key, mode))
    expected = reference[key]
    assert len(expected) == len(row[7:]) == 16
    error = max(abs(a - b) for a, b in zip(row[7:], expected))
    groups.setdefault((int(key[0]), mode), []).append(error)
assert len(seen) == 576
print(
    json.dumps(
        [
            dict(
                width=w,
                rounding=bool(r),
                cases=len(v),
                within1=sum(x <= 1 for x in v),
                withinQuarter=sum(x <= 0.25 for x in v),
                maxError=max(v),
            )
            for (w, r), v in sorted(groups.items())
        ],
        indent=2,
    )
)
