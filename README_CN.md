# magnet

[English](README.md)

用 Rust 编写的多 Provider torrent 搜索 CLI，同时提供可复用的 library。支持并发搜索、BTIH infohash 去重、tracker 合并，以及适合脚本和 agent 调用的 JSON 输出。

版本发布在 [crates.io](https://crates.io/crates/magnet-cli)。内置 Provider 为 Nyaa、Knaben、Sukebei（Nyaa NSFW）、Bitsearch、BtGoogle、APIBay、DMHY、Anime Garden 和 Mikan Project，另可配置 Torznab 与 RSS/Atom。搜索结果可以交给其他 BitTorrent 客户端；本项目不包含下载器、TUI 或 MCP server。

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
| `--no-title-filter` | 关闭 | 保留标题缺少查询词的结果 |

`--json`、`--jsonl`、`--magnet` 互斥。未选择时输出表格，未知的 seeders、大小和日期显示为 `?`，大小显示为十进制 GB，AGE 以天计。

大小可直接写字节数，也可使用 K/M/G/T、KB/MB/GB/TB（十进制）或 KiB/MiB/GiB/TiB（二进制），例如 `10G`、`1.5GiB`。设置大小或 seeders 过滤器后，对应字段未知的结果会被排除。过滤在去重之后执行，`--limit` 在排序之后执行。

### 全局参数与环境变量

| 参数 | 环境变量 | 作用 |
| --- | --- | --- |
| `--config PATH` | `MAGNET_CONFIG` | 指定 TOML 配置文件 |
| `--cache PATH` | `MAGNET_CACHE` | 指定搜索快照文件，便于隔离会话 |

命令行参数优先于对应环境变量。全局参数可以放在子命令前后。程序不会自动读取当前目录的 `config.toml`；需要显式指定路径或设置 `MAGNET_CONFIG`。

## 文档导航

| 文档 | 内容 |
| --- | --- |
| [使用教程](docs/usage.zh-CN.md) | 分步教程：安装、命令、过滤、输出、选取某条磁链 |
| [Provider 配置](docs/providers.zh-CN.md) | 自定义源、`kind` 一览、Sukebei、Torznab、RSS |
| [脚本与集成](docs/scripting.zh-CN.md) | JSON 结构、聚合规则、标题过滤、快照与 `get`、退出码、pikpaktui |
| [文档索引](docs/README_CN.md) | `docs/` 下的全部文档与状态 |
| [CHANGELOG](CHANGELOG.md) · [RELEASING](RELEASING.md) | 发布历史与发布流程 |

## 当前限制

默认最多同时执行 8 个 Provider（可配置为 1–64），每个源默认 15 秒超时，单次响应上限 8 MiB。更多源会排队；`--timeout` 覆盖一个 Provider 及其所有请求页，需要限制整条命令时使用 `--deadline`。

每个源默认读取一页。`--pages` 可为 Knaben、Bitsearch 和 Torznab 请求最多 20 页；APIBay、Nyaa/Sukebei RSS 与通用 RSS/Atom 只请求一次。`--limit` 只限制最终输出数量，不保证能搜满指定条数。公网源的可用性、限流和结果完整性取决于上游服务。

BtGoogle 是唯一一个通过抓取 HTML 而非调用文档化 API 的 Provider，因此该页面结构变化时它最可能失效。当前未实现 TUI、MCP、DHT 查询或下载功能。

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
    ├── feeds.rs        # Nyaa / Sukebei / Torznab / RSS / DMHY / Mikan
    ├── knaben.rs       # Knaben JSON API
    ├── apibay.rs       # APIBay JSON API
    ├── bitsearch.rs    # Bitsearch JSON API
    ├── btgoogle.rs     # BtGoogle HTML 搜索端点
    └── animegarden.rs  # Anime Garden JSON API
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
