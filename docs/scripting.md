# Scripting & integration

[中文](scripting.zh-CN.md)

This page collects the JSON result shape, aggregation rules, search snapshots, exit codes, and the scripting and pikpaktui integration patterns. For a quick tour see the [usage tutorial](usage.md) and for the overview see the [README](../README.md).

## JSON result shape

`search --json` returns an array; `--jsonl` uses the same object shape. Sample data:

```json
[
  {
    "id": 1,
    "title": "Example",
    "info_hash": "0000000000000000000000000000000000000000",
    "magnet": "magnet:?xt=urn%3Abtih%3A0000000000000000000000000000000000000000&dn=Example",
    "size": null,
    "seeders": null,
    "leechers": null,
    "published_at": null,
    "sources": ["local"],
    "detail_url": null,
    "trackers": []
  }
]
```

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | integer | Position in the current search snapshot, from 1; local `resolve` prints 0 |
| `title` | string | Torrent title |
| `info_hash` | string or null | BTIH normalized to lowercase hex |
| `magnet` | string or null | URL-encoded magnet URI |
| `size` | integer or null | Bytes |
| `seeders` / `leechers` | integer or null | Seeder / leecher counts as reported |
| `published_at` | string or null | UTC RFC 3339 timestamp; null when unparseable or zone-less |
| `sources` | string array | Merged provider names, sorted and deduped |
| `detail_url` | string or null | Detail or related link from the source |
| `trackers` | string array | Merged tracker URLs, sorted and deduped |

`get --json` and `resolve --json` print a single object; `providers --json` prints an array with `name`, `kind`, and `default_search`.

## Aggregation rules

Aggregation keys on BTIH, never on title. Base32 hashes convert to hex; hash-less results never merge even with identical names. For one hash, sources and trackers union, seeders/leechers take the max reported value, never summed.

The title comes from the first record in config order; size, publish time, and detail link keep that record's values, backfilled from later records when missing. Provider completion order doesn't affect this priority. Invalid or conflicting hashes are skipped with a warning.

Rebuilt magnets keep BTIH, display name, and trackers; other magnet params are dropped. BTIH/v1 is supported; v2-only magnets are not.

## Title filtering

Public indexes tokenize CJK queries per character, and when a query has no real match they pad the page with their most popular rows. Search therefore drops any result whose title does not contain every whitespace-separated term of the query, ignoring case. This runs per provider, after magnet normalization and before dedup, and covers every query — not just non-ASCII ones. Each drop is reported on stderr as `NAME: dropped N result(s) that do not match the query`, so filtering is never silent.

The check is a plain substring test. It is deliberately predictable rather than clever, which means a query spelled differently from the titles it should match can be filtered out: searching `C++` drops `Microsoft Visual C+ Redistributable` while keeping `Visual C++ [2005-2015]`. Pass `--no-title-filter` when a query is written in a form the titles never use literally.

## Search snapshots & `get`

Results hit the snapshot after filtering, sorting, and truncation, then print. Linux default is `~/.cache/magnet/last-search.json`; with `XDG_CACHE_HOME` set it lives under `magnet/last-search.json` there. Other platforms use the platform cache dir; when none is found it falls back to `./.magnet-last-search.json`.

`get ID` only reads the snapshot, never the network. IDs are valid for that one search; the next search renumbers. Any search reaching the snapshot stage — empty results and all-provider-failed included — atomically replaces the old snapshot; argument validation failures don't wipe it, write failures exit 2.

Records without hash/magnet can still appear in tables and JSON. `get ID` on one exits 1, while `get ID --json` reads the full record. A missing or corrupt cache file exits 2; search again.

Concurrent searches use the last successfully written snapshot. Scripts or agent sessions can isolate the cache explicitly:

```bash
export MAGNET_CACHE="$PWD/session-search.json"
magnet search "ubuntu" --json
magnet get 1 --json
```

## Exit codes

Results go to stdout, warnings and errors to stderr. Scripts should capture both separately and check the exit code.

| Code | Meaning |
| --- | --- |
| `0` | Search returned results, or other command succeeded |
| `1` | No matches, no such ID in the snapshot, or the result has no printable magnet |
| `2` | All sources failed including parse/runtime config errors, or local I/O failed |
| `3` | Bad arguments or config, including unknown source, unreadable config file |
| `4` | Every provider failed with HTTP, network, or timeout errors |

Partial source failures print warnings; the exit stays 0 while results remain. At least one success but nothing after filtering exits 1. All failed with mixed network and parse errors exits 2. A missing `api_key_env` variable counts as that provider's runtime failure.

A search reaching the output stage prints `[]` for empty JSON and zero lines for empty JSONL/magnet; on argument, config, or cache-write failure, stdout JSON is not guaranteed. JSONL emits once after aggregation, not as a per-provider stream.

## Python example

The example below needs no extra dependencies and tells "no results" apart from failure:

```python
import json
import subprocess
import sys

result = subprocess.run(
    ["magnet", "--cache", "session-search.json", "search", "ubuntu", "--json"],
    capture_output=True,
    text=True,
)
if result.stderr:
    print(result.stderr, end="", file=sys.stderr)
if result.returncode not in (0, 1):
    raise SystemExit(result.returncode)
rows = json.loads(result.stdout)
for row in rows:
    if row["magnet"]:
        print(row["magnet"])
```

## Picking one record's magnet

- To eyeball and pick: use the snapshot ID, `magnet get ID` (exits 1 when there is no magnet).
- To select by rule in a script: use `--json` + `jq`.

```bash
# By position
magnet search "ubuntu" --json | jq -r '.[0].magnet'
# By id field
magnet search "ubuntu" --json | jq -r '.[] | select(.id==3) | .magnet'
# By title keyword
magnet search "ubuntu" --json | jq -r '.[] | select(.title | test("22.04")) | .magnet'
# Highest seeders (handles null)
magnet search "ubuntu" --json | jq -r 'sort_by(-(.seeders // 0))[0].magnet'
# Line N of --magnet
magnet search "ubuntu" --magnet | sed -n '3p'
```

The safer extraction confirms success and a valid magnet first:

```bash
if results=$(magnet search "ubuntu" --json) &&
   uri=$(printf '%s' "$results" | jq -er '.[0].magnet | select(type == "string" and startswith("magnet:?"))'); then
    echo "$uri"
fi
```

## Piping into pikpaktui

Install `jq` and `pikpaktui` and finish `pikpaktui` login first. Pipe the first magnet of a search into `pikpaktui offline` to create a PikPak cloud task:

```bash
magnet search "odv-534" --json | jq -r '.[0].magnet' | xargs pikpaktui offline
```

`.[0]` picks the top result (seeders descending by default), `jq -r` strips the JSON quotes, `xargs` passes the magnet as the argument.

For scripts, confirm success first, then extract a non-empty magnet and quote the full URI. Nothing is created when the search fails, results are empty, or the first row lacks a magnet:

```bash
if results=$(magnet search "odv-534" --json) &&
   uri=$(printf '%s' "$results" | jq -er '.[0].magnet | select(type == "string" and startswith("magnet:?"))'); then
    pikpaktui offline "$uri"
fi
```

Or read a picked ID from the last search:

```bash
if uri=$(magnet get 1); then
    pikpaktui offline "$uri" --to "/Downloads"
fi
```

`pikpaktui offline` takes `--to` for the target dir, `--name` for the task name, and `--dry-run` to preview; e.g. swap the last call for `pikpaktui offline "$uri" --dry-run`. Accounts, target dirs, and offline tasks belong to `pikpaktui`; `magnet` handles search and magnet output.

## Piping into fzf

Install `jq` and `fzf`. Turn each result into a tab-separated `seeders<TAB>size<TAB>title<TAB>magnet` row, let fzf show only the first three columns, then `cut -f4` the selected row's magnet:

```bash
magnet search "ubuntu" --json \
  | jq -r '.[] | select(.magnet != null)
      | [ (.seeders // "-" | tostring),
          (if .size then ((.size / 1000000000 * 100 | round) / 100 | tostring) + " GB" else "?" end),
          (.title | gsub("[\\t\\n\\r]"; " ")),
          .magnet ]
      | @tsv' \
  | fzf --delimiter='\t' --with-nth=1,2,3 --nth=3 --reverse \
        --preview 'printf "%s\n" {4}' --preview-window=down:3:wrap \
  | cut -f4
```

- `@tsv` splits fields on tabs; `gsub` strips tabs and newlines from titles so they can't break the columns.
- `--with-nth=1,2,3` shows only seeders, size, and title; the magnet stays as a hidden 4th column and still travels with the row.
- `--nth=3` restricts fuzzy matching to the title; drop it to match all four fields.
- `--preview` uses fzf's `{4}` placeholder (which fzf single-quotes for you) to reveal the hidden full magnet.
- `cut -f4` prints the selected row's magnet. Because `select(.magnet != null)` ran first, a selected row always has one.

Wrap it in a reusable function:

```bash
magnet-pick() {
  magnet search "$*" --json \
    | jq -r '.[] | select(.magnet != null)
        | [ (.seeders // "-" | tostring),
            (if .size then ((.size / 1000000000 * 100 | round) / 100 | tostring) + " GB" else "?" end),
            (.title | gsub("[\\t\\n\\r]"; " ")),
            .magnet ]
        | @tsv' \
    | fzf --delimiter='\t' --with-nth=1,2,3 --nth=3 --reverse \
          --prompt='torrent> ' --preview 'printf "%s\n" {4}' --preview-window=down:3:wrap \
    | cut -f4
}
```

```bash
# Pick interactively, then print the magnet
magnet-pick "ubuntu"

# Pick interactively, then create a PikPak cloud task
uri=$(magnet-pick "odv-534") && [ -n "$uri" ] && pikpaktui offline "$uri"
```

In a script with no terminal, `fzf --filter` performs the same fuzzy match non-interactively:

```bash
magnet search "ubuntu" --json \
  | jq -r '.[] | select(.magnet != null)
      | [ (.seeders // "-" | tostring),
          (if .size then ((.size / 1000000000 * 100 | round) / 100 | tostring) + " GB" else "?" end),
          (.title | gsub("[\\t\\n\\r]"; " ")),
          .magnet ]
      | @tsv' \
  | fzf --delimiter='\t' --with-nth=1,2,3 --nth=3 --filter='22.04' \
  | cut -f4
```

Note: `magnet-pick` runs a fresh search and rewrites the snapshot every time. To pick several times from one result set, search once and pin the snapshot with `MAGNET_CACHE`, or use `fzf --multi` to select multiple rows at once (then `cut -f4` prints one magnet per line).
