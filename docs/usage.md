# magnet-cli usage tutorial

[中文](usage.zh-CN.md)

A step-by-step tour for everyday use. For the full option and configuration reference see [Provider configuration](providers.md) and [Scripting & integration](scripting.md); for the overview see the [README](../README.md).

## 1. Install

Install from crates.io (the command is `magnet`):

```bash
cargo install magnet-cli --locked
magnet --version
magnet search "ubuntu" --json
```

Or install from source:

```bash
git clone https://github.com/wustites/magnet-cli.git
cd magnet-cli
cargo install --path . --locked
```

The installed command is `magnet`. If your shell can't find it, make sure Cargo's bin directory is on `PATH` (usually `~/.cargo/bin`). You can also just build and run inside the repo:

```bash
cargo build --release --locked
./target/release/magnet search "ubuntu"
```

## 2. Four subcommands

| Command | What it does |
| --- | --- |
| `search "term"` | Queries several sources concurrently, then aggregates and dedups |
| `get ID` | Reads entry N (1-based) from the last search snapshot; no network |
| `resolve HASH` | Builds a magnet locally from an infohash; no DHT, no metadata |
| `providers` | Lists configured sources (no URLs or keys); no search request |

## 3. Basic search

```bash
# Default sources, top 20 by seeders descending
magnet search "ubuntu"

# Pick sources (comma-separated or repeated --source)
magnet search "ubuntu" --source knaben --json
magnet search "ubuntu" --source nyaa,knaben --jsonl

# One magnet per line, good for piping into scripts
magnet search "ubuntu" --magnet
```

With no config, Nyaa, Knaben, Sukebei, Bitsearch, and BtGoogle run by default. APIBay, DMHY, Anime Garden, and Mikan are opt-in via `--source`. Differences between built-in sources are covered in [Provider configuration](providers.md).

## 4. Filtering, sorting, and count

```bash
magnet search "ubuntu" \
  --min-seeds 10 --min-size 1G --max-size 10G \
  --sort seeds --limit 20
```

- `--sort seeds|size|date|title`: the first three descend, unknowns last; title ascends.
- Sizes accept `10G`, `1.5GiB`, or raw bytes.
- Once a seeders or size filter is set, results with that field unknown are excluded.
- The order is: **dedup by BTIH → filter → sort → `--limit`**.

## 5. Output formats

`--json`, `--jsonl`, and `--magnet` are mutually exclusive; with none, you get a table.

- `--json`: a JSON array.
- `--jsonl`: one JSON object per line, emitted once after aggregation.
- `--magnet`: one magnet per line, skipping results without one.

In the table, unknown seeders, size, and date show as `?`, sizes render as decimal GB, and AGE counts days. Field meanings are in [Scripting & integration](scripting.md).

## 6. Global options & environment variables

| Option | Env var | Effect |
| --- | --- | --- |
| `--config PATH` | `MAGNET_CONFIG` | Use a TOML config file |
| `--cache PATH` | `MAGNET_CACHE` | Use a snapshot file, handy for isolated sessions |

CLI flags win over the matching env vars, and global options can go before or after the subcommand. The program never auto-reads `./config.toml`; pass a path explicitly or set `MAGNET_CONFIG`.

## 7. Search snapshots & `get`

A search writes its results to a snapshot (Linux default `~/.cache/magnet/last-search.json`). `get ID` only reads the snapshot, never the network, and IDs are valid for that one search.

```bash
magnet get 1          # prints the magnet of entry 1
magnet get 1 --json   # prints the full JSON object of entry 1

# Isolate the cache for a script or agent session
export MAGNET_CACHE="$PWD/session-search.json"
magnet search "ubuntu" --json
magnet get 1 --json
```

More detail (snapshot replacement rules, records without a magnet) is in [Scripting & integration](scripting.md).

## 8. `resolve`

```bash
magnet resolve 0000000000000000000000000000000000000000 --name "Example" --json
```

It accepts a 40-char hex or 32-char base32 BTIH, plus repeatable `--tracker URL`. It only validates the hash and builds a magnet: no DHT lookup, no detail-page fetch, no torrent metadata. The all-zero hash is demo-only.

## 9. Picking one record's magnet

Three common approaches, depending on how you want to locate the record.

**Approach 1: snapshot ID + `get` (closest to the CLI)**

Every result has a 1-based `id`, the same one shown in the table or JSON:

```bash
magnet search "ubuntu"   # read the results, note the ID you want
magnet get 3             # prints the magnet of entry 3
magnet get 3 --json      # prints the full JSON of entry 3
```

The ID is valid only for the last search; a new search renumbers. If that record has no magnet, `get ID` exits `1` while `get ID --json` still reads the full record.

**Approach 2: `--json` + `jq` (most flexible)**

```bash
# By position (entry 1, the highest-seeders row by default)
magnet search "ubuntu" --json | jq -r '.[0].magnet'

# By id field
magnet search "ubuntu" --json | jq -r '.[] | select(.id==3) | .magnet'

# By title keyword
magnet search "ubuntu" --json | jq -r '.[] | select(.title | test("22.04")) | .magnet'

# Manually take the highest seeders (handles null)
magnet search "ubuntu" --json | jq -r 'sort_by(-(.seeders // 0))[0].magnet'
```

**Approach 3: line N of `--magnet`**

```bash
magnet search "ubuntu" --magnet | sed -n '3p'
```

**The safe script pattern**: confirm the search succeeded and the target actually has a valid magnet before using it.

```bash
if results=$(magnet search "ubuntu" --json) &&
   uri=$(printf '%s' "$results" | jq -er '.[0].magnet | select(type == "string" and startswith("magnet:?"))'); then
    echo "$uri"   # replace with your client, e.g. pikpaktui offline "$uri"
fi
```

In one line: **to eyeball and pick, use `get ID`; to select by rule in a script, use `--json` + `jq`.**

## 10. Configuring custom sources

```bash
cp config.example.toml config.toml
magnet --config config.toml providers --json
magnet --config config.toml search "ubuntu" --json
```

An explicit config **replaces** the default source list, it is not appended. Each `[[providers]]` entry needs `name`, `kind`, and an HTTP(S) `url`; `default_search = false` keeps a source available only via `--source`. Full fields, per-`kind` behavior, and Torznab / RSS examples are in [Provider configuration](providers.md).

## 11. Exit codes & scripting

Results go to stdout, warnings and errors to stderr, so scripts should capture both and check the exit code:

| Code | Meaning |
| --- | --- |
| `0` | Results returned, or another command succeeded |
| `1` | No matches, no such ID in the snapshot, or the result has no printable magnet |
| `2` | All sources failed (including parse/runtime config errors), or local I/O failed |
| `3` | Bad arguments or config, e.g. unknown source, unreadable config |
| `4` | Every provider failed with HTTP, network, or timeout errors |

Full Python/Bash examples and the pikpaktui integration are in [Scripting & integration](scripting.md).

## 12. Tips & limits

- Title filtering: results whose title omits any query term (case-insensitive) are dropped by default, and every drop is reported on stderr. Pass `--no-title-filter` when a query is spelled in a way the titles never use literally (e.g. `C++`).
- CJK queries: public sources tend to return filler rows; add Knaben or Bitsearch for coverage. APIBay is archived from default searches for CJK.
- Up to 8 providers run concurrently with a 15 s default timeout. Tune with `--concurrency` (1–64), `--timeout` (1–300), `--deadline` (1–3600), and `--pages` (1–20).
- BtGoogle scrapes HTML and is the most likely to break on a page change. There is no downloader, TUI, DHT lookup, or MCP server.

## Related

- [Provider configuration](providers.md)
- [Scripting & integration](scripting.md)
- [Documentation index](README.md)
