# magnet

[English](README.md)

用 Rust 编写的多 Provider torrent 搜索 CLI，同时提供可复用的 library。支持并发搜索、BTIH infohash 去重、tracker 合并，以及适合脚本和 agent 调用的 JSON 输出。

版本发布在 [crates.io](https://crates.io/crates/magnet-cli)。0.2.0 内置 Nyaa、Knaben、Sukebei（Nyaa NSFW）、APIBay 和 Bitsearch，可配置 Torznab 与 RSS/Atom。搜索结果可以交给其他 BitTorrent 客户端；本项目不包含下载器、TUI 或 MCP server。

## 安装与快速开始

从 crates.io 安装（命令名是 `magnet`）：

```bash
cargo install magnet-cli --locked
magnet --version
magnet search "ubuntu" --json
```

也可以从源码安装：

```bash
git clone https://github.com/wustites/magnet-cli.git
cd magnet-cli
cargo install --path . --locked
```

安装后的命令名是 `magnet`。如果 shell 找不到命令，请确认 Cargo 的 bin 目录在 `PATH` 中（默认通常为 `~/.cargo/bin`）。

也可以只构建，在仓库内直接运行：

```bash
cargo build --release --locked
./target/release/magnet search "ubuntu"
```

## 常用命令

```bash
# 默认查询参与默认搜索的源，按 seeders 降序显示前 20 条
magnet search "ubuntu"

# 指定源；支持逗号分隔或重复 --source
magnet search "ubuntu" --source knaben --json
magnet search "ubuntu" --source nyaa,knaben --jsonl

# 每行一个 magnet，适合传给脚本
magnet search "ubuntu" --magnet

# 过滤、排序与结果数量
magnet search "ubuntu" --min-seeds 10 --min-size 1G --max-size 10G --sort seeds --limit 20

# 读取上一次搜索的结果，ID 从 1 开始
magnet get 1
magnet get 1 --json

# 查看已配置的 Provider，不发起搜索请求
magnet providers --json

# 从 infohash 本地构造 magnet；全零 hash 仅用于演示
magnet resolve 0000000000000000000000000000000000000000 --name "Example" --json
```

`resolve` 接受 40 位十六进制或 32 位 base32 BTIH，可重复传入 `--tracker URL`。它只校验 hash 并构造 magnet，不查询 DHT、不解析详情页，也不获取 torrent 元数据。`get --json` 和 `resolve --json` 输出单个对象；`providers --json` 输出包含 `name`、`kind`、`default_search` 的数组。

使用 `magnet --help` 或 `magnet search --help` 查看命令帮助。

### 搜索参数

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `QUERY` | 必填 | 非空搜索词；含空格时加引号 |
| `--source NAME` | `default_search = true` 的源 | 按 Provider 的 `name` 选择，未知名称返回退出码 3 |
| `--json` | 关闭 | 输出 JSON 数组 |
| `--jsonl` | 关闭 | 每行一个 JSON 对象，在聚合完成后输出 |
| `--magnet` | 关闭 | 每行一个 magnet，排除无法构造 magnet 的结果 |
| `--min-seeds N` | 不限制 | seeders 下限，包含边界 |
| `--min-size SIZE` | 不限制 | 大小下限，包含边界 |
| `--max-size SIZE` | 不限制 | 大小上限，包含边界 |
| `--sort seeds\|size\|date\|title` | `seeds` | 前三种降序、未知值排最后；标题升序 |
| `--limit N` | `20` | 最终聚合结果的上限，必须大于 0 |
| `--timeout SECONDS` | `15` | 每个 Provider 的超时，范围 1–300 秒 |
| `--concurrency N` | `8` | 同时搜索的 Provider 数量，范围 1–64 |
| `--deadline SECONDS` | 不限制 | 整次搜索的总截止时间（包含排队），范围 1–3600 秒 |
| `--pages N` | `1` | 向支持分页的 Provider 请求的页数，范围 1–20 |

`--json`、`--jsonl`、`--magnet` 互斥。未选择时输出表格，未知的 seeders、大小和日期显示为 `?`，大小显示为十进制 GB，AGE 以天计。

大小可直接写字节数，也可使用 K/M/G/T、KB/MB/GB/TB（十进制）或 KiB/MiB/GiB/TiB（二进制），例如 `10G`、`1.5GiB`。设置大小或 seeders 过滤器后，对应字段未知的结果会被排除。过滤在去重之后执行，`--limit` 在排序之后执行。

### 全局参数与环境变量

| 参数 | 环境变量 | 作用 |
| --- | --- | --- |
| `--config PATH` | `MAGNET_CONFIG` | 指定 TOML 配置文件 |
| `--cache PATH` | `MAGNET_CACHE` | 指定搜索快照文件，便于隔离会话 |

命令行参数优先于对应环境变量。全局参数可以放在子命令前后。程序不会自动读取当前目录的 `config.toml`；需要显式指定路径或设置 `MAGNET_CONFIG`。

## 配置 Provider

没有指定配置时，默认并发查询 Nyaa RSS、Knaben JSON、Sukebei RSS、APIBay 和 Bitsearch。显式配置会**替换默认 Provider 列表**，不会追加到默认列表。

从 [config.example.toml](config.example.toml) 开始：

```bash
cp config.example.toml config.toml
magnet --config config.toml providers --json
magnet --config config.toml search "ubuntu" --json
```

每个 `[[providers]]` 条目必须包含 `name`、`kind` 和 HTTP(S) `url`。`name` 必须非空且唯一，只能使用 ASCII 字母、数字、连字符或下划线。至少配置一个源；未知配置字段会被拒绝。`api_key_env` 仅适用于 Torznab 和 Bitsearch，且必须是合法的环境变量名。

| `kind` | 请求方式 | 搜索行为 |
| --- | --- | --- |
| `nyaa` | HTTP GET RSS | 添加 `page=rss` 和搜索参数 `q` |
| `sukebei` | HTTP GET RSS | 使用 Nyaa RSS 协议，添加 `page=rss` 和 `q` |
| `knaben` | HTTP POST JSON | 向配置的 API URL 提交标题搜索，单次请求 150 条 |
| `apibay` | HTTP GET JSON | 在 APIBay 的全部分类中搜索；读取 API 固定返回的结果集 |
| `bitsearch` | HTTP GET JSON | 搜索 Bitsearch，每页 100 条；可选 API key 请求头 |
| `torznab` | HTTP GET RSS/XML | 添加 `t=search`、`q`、`extended=1`，可带 API key |
| `rss` | HTTP GET RSS/Atom | 原样请求 URL，在本地按标题过滤；搜索词按空白拆分，忽略大小写且每个词都必须匹配 |

`--pages` 分别使用 Knaben 的 `from` 偏移、Bitsearch 的 `page` 和 Torznab 的 `offset`/`limit`。APIBay、Nyaa/Sukebei RSS 与通用 RSS/Atom 没有可靠的兼容分页机制，因此只请求一次。Provider 返回空页或末页时提前停止；单 Provider 超时包含其请求的所有页面。

Bitsearch 匿名额度目前为每个 IP 每日 200 次。需要使用账号 API key 时，在该 Provider 上设置 `api_key_env`；密钥通过 `x-api-key` 请求头发送，不会出现在诊断信息中。

### Sukebei：Nyaa NSFW

```bash
magnet search "关键词" --source sukebei --json
magnet search "关键词" --source nyaa,sukebei --jsonl
```

Sukebei 使用 `https://sukebei.nyaa.si/`。自定义配置可加入以下条目：

```toml
[[providers]]
name = "sukebei"
kind = "sukebei"
url = "https://sukebei.nyaa.si/"
default_search = true
```

所有 Provider 都支持可选字段 `default_search`（省略时为 `true`）。设为 `false` 时仍会出现在 `providers` 列表中，但仅在 `--source` 显式选择时参与搜索。内置 Sukebei 和示例配置将其设为 `true`，参与默认聚合搜索；改为 `false` 可排除默认搜索。若没有可用于默认搜索的源，须传入 `--source`，否则返回 3。

### Torznab：Jackett / Prowlarr

在配置文件中添加以下条目，或取消示例文件中对应条目的注释。将 `url` 替换为服务给出的**完整 Torznab API endpoint**，不能只填服务首页：

```toml
[[providers]]
name = "local"
kind = "torznab"
url = "http://localhost:9117/api/v2.0/indexers/all/results/torznab/api"
api_key_env = "TORZNAB_API_KEY"
```

`api_key_env` 是保存密钥的环境变量名称，而不是密钥本身；仅 Torznab 使用此字段。不需要鉴权的源可以省略它。

```bash
export TORZNAB_API_KEY='your-key'
magnet --config config.toml search "ubuntu" --source local --json
```

`providers` 只显示名称和类型，不显示 URL 或密钥。HTTP 错误诊断也不输出请求 URL。

### RSS

```toml
[[providers]]
name = "linux"
kind = "rss"
url = "https://example.org/torrents.rss"
```

上面的 URL 是占位示例，需替换为实际 RSS 或 Atom feed。解析器支持 RSS item 和 Atom entry 中的 magnet 链接、enclosure，以及 Nyaa/Torznab 扩展字段。仅有 `.torrent` 下载链接时，不会自动下载文件计算 hash。普通 Newznab NZB 结果不能转换成 BitTorrent magnet。

协议参考：[Knaben API](https://knaben.org/api/v1/)、[Bitsearch API](https://bitsearch.eu/api)、[Nyaa RSS 模板](https://github.com/nyaadevs/nyaa/blob/master/nyaa/templates/rss.xml)、[Torznab 规范](https://torznab.github.io/spec-1.3-draft/torznab/Specification-v1.3.html)。

## JSON 结果与聚合规则

`search --json` 返回数组，`--jsonl` 使用相同的对象结构。以下为示例数据：

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

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | 整数 | 当前搜索快照中的位置，从 1 开始；本地 `resolve` 输出为 0 |
| `title` | 字符串 | torrent 标题 |
| `info_hash` | 字符串或 null | 规范化为小写十六进制的 BTIH |
| `magnet` | 字符串或 null | URL 编码后的 magnet URI |
| `size` | 整数或 null | 字节数 |
| `seeders` / `leechers` | 整数或 null | 源报告的做种者 / 下载者数量 |
| `published_at` | 字符串或 null | UTC RFC 3339 时间；无法解析或缺少时区时为 null |
| `sources` | 字符串数组 | 已合并结果的 Provider 名称，排序并去重 |
| `detail_url` | 字符串或 null | 源提供的详情或关联链接 |
| `trackers` | 字符串数组 | 合并后的 tracker URL，排序并去重 |

聚合以 BTIH 为主键，不使用标题。base32 hash 会转换成十六进制；没有 hash 的结果即使同名也不会合并。同一 hash 的 sources、trackers 取并集，seeders、leechers 取最大报告值，不累加。

标题采用配置顺序中最先出现的记录；大小、发布时间和详情链接优先保留该记录的已有值，缺失时从后续记录补齐。Provider 完成请求的先后顺序不影响这个优先级。无效或冲突的 hash 会被跳过并输出警告。

重建 magnet 时保留 BTIH、显示名称和 tracker，其他 magnet 参数不保留。当前支持 BTIH/v1，不支持仅包含 v2 hash 的 magnet。

## 搜索快照与 `get`

搜索结果在过滤、排序和截断后写入快照，再输出到终端。Linux 默认路径是 `~/.cache/magnet/last-search.json`；设置 `XDG_CACHE_HOME` 时使用该目录下的 `magnet/last-search.json`。其他平台使用平台用户缓存目录；无法确定目录时回退到当前目录的 `.magnet-last-search.json`。

`get ID` 只读取快照，不访问网络。ID 仅对这一次搜索有效，后续搜索会重新编号。执行到快照写入阶段的搜索，包括空结果和所有 Provider 失败的搜索，都会原子替换旧快照；参数校验失败不会清空旧快照，写入失败则返回退出码 2。

没有 hash/magnet 的记录仍可出现在表格和 JSON 中。对此执行 `get ID` 返回 1，`get ID --json` 可以读取完整记录。缓存文件缺失或损坏时返回 2，请重新搜索。

并发搜索使用最后一次成功写入的快照。脚本或 agent 会话可显式隔离缓存：

```bash
export MAGNET_CACHE="$PWD/session-search.json"
magnet search "ubuntu" --json
magnet get 1 --json
```

## 退出码与脚本调用

结果写入 stdout，警告和错误写入 stderr。脚本应分别捕获两者，并检查退出码。

| 退出码 | 含义 |
| --- | --- |
| `0` | 搜索返回结果，或其他命令成功 |
| `1` | 无匹配结果、快照中没有指定 ID，或该结果没有可输出的 magnet |
| `2` | 所有源失败且包含解析/运行时配置错误，或本地 I/O 失败 |
| `3` | 无效参数或配置，包括未知 source、无法读取配置文件 |
| `4` | 所有 Provider 都因 HTTP、网络或超时错误失败 |

部分源失败时会输出警告；只要最终有结果，退出码仍为 0。至少一个源成功但过滤后无结果时返回 1。全部源失败且混合网络和解析错误时返回 2。缺失 `api_key_env` 指定的环境变量属于该 Provider 的运行时失败。

搜索正常执行到输出阶段时，空 JSON 是 `[]`，空 JSONL/magnet 没有任何行；参数、配置或缓存写入失败时，不保证 stdout 中有 JSON。JSONL 在聚合完成后一次输出，不是逐 Provider 实时流。

下面的 Python 示例无需额外依赖，并区分无结果与执行失败：

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

## 与 pikpaktui 通过命令行集成

安装 `jq` 和 `pikpaktui`，并先完成 `pikpaktui` 的登录配置。用管道将搜索结果中的第一条 magnet 传给 `pikpaktui offline`，即可创建 PikPak 云端离线任务：

```bash
magnet search "odv-534" --json | jq -r '.[0].magnet' | xargs pikpaktui offline
```

`.[0]` 选择排序后的第一条结果（默认按 seeders 降序），`jq -r` 去掉 JSON 字符串的引号，`xargs` 将 magnet 作为 `pikpaktui offline` 的参数。

在脚本中可使用下面的 Bash 写法：先确认搜索成功，再提取非空 magnet，并用引号传递完整 URI。搜索失败、结果为空或第一条没有 magnet 时，不会创建任务。

```bash
if results=$(magnet search "odv-534" --json) &&
   uri=$(printf '%s' "$results" | jq -er '.[0].magnet | select(type == "string" and startswith("magnet:?"))'); then
    pikpaktui offline "$uri"
fi
```

也可以读取上一次搜索中选定的 ID：

```bash
if uri=$(magnet get 1); then
    pikpaktui offline "$uri" --to "/Downloads"
fi
```

`pikpaktui offline` 支持 `--to` 指定目标目录、`--name` 指定任务名称，以及 `--dry-run` 预览而不创建任务；例如将最后一行调用改为 `pikpaktui offline "$uri" --dry-run`。账号、目标目录和离线任务均由 `pikpaktui` 管理，`magnet` 负责搜索和输出 magnet。

## 当前限制

默认同时执行 8 个 Provider（可配置为 1–64），每个源默认 15 秒超时，单次响应上限 8 MiB。更多源会排队；`--timeout` 覆盖一个 Provider 及其所有请求页，需要限制整条命令时使用 `--deadline`。

每个源默认读取一页。`--pages` 可为 Knaben、Bitsearch 和 Torznab 请求最多 20 页；APIBay、Nyaa/Sukebei RSS 与通用 RSS/Atom 只请求一次。`--limit` 只限制最终输出数量，不保证能搜满指定条数。公网源的可用性、限流和结果完整性取决于上游服务。

当前未实现 TUI、MCP、HTML 抓取、DHT 查询或下载功能。

## 开发与扩展

```text
src/
├── main.rs             # 命令执行、输出、过滤和排序
├── cli.rs              # clap 参数定义
├── model.rs            # Torrent 模型、hash/magnet 规范化
├── search.rs           # 并发调度、错误汇总、去重
├── cache.rs            # 原子写入和读取搜索快照
├── lib.rs              # library 模块导出
└── providers/
    ├── mod.rs          # Provider trait、配置和 HTTP 请求封装
    ├── feeds.rs        # Nyaa / Sukebei / Torznab / RSS
    └── knaben.rs       # Knaben JSON API
```

crate 根目录重新导出 `Provider`、`Torrent`、`SearchOptions` 和两个搜索入口，可供新 Provider 或其他前端复用。基础自定义源只需实现 `Provider::search`，支持分页时可覆盖 `search_pages`；兼容旧行为时调用 `search`，需要并发、总截止时间和分页控制时调用 `search_with_options`。要让 CLI 的 TOML 配置支持新的 `kind`，还需更新 `Kind` 和 `HttpProvider` 分发逻辑。

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

测试使用本地 HTTP 模拟服务，不需要公网 Provider 或 API key。CI 执行格式检查、Clippy 和测试。

## 许可证

[MIT](LICENSE)
