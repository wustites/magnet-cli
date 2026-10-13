# magnet-cli 使用教程

[English](usage.md)

面向日常使用的分步教程。完整的参数与配置说明见 [Provider 配置](providers.zh-CN.md) 和 [脚本与集成](scripting.zh-CN.md)；总览见 [README_CN](../README_CN.md)。

## 1. 安装

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

安装后的命令名是 `magnet`。如果 shell 找不到命令，请确认 Cargo 的 bin 目录在 `PATH` 中（默认通常为 `~/.cargo/bin`）。也可以只构建并在仓库内运行：

```bash
cargo build --release --locked
./target/release/magnet search "ubuntu"
```

## 2. 四个子命令

| 命令 | 作用 |
| --- | --- |
| `search "词"` | 并发查询多个源，聚合去重后输出 |
| `get ID` | 读取上次搜索快照中的第 N 条（从 1 开始），不联网 |
| `resolve HASH` | 用 infohash 本地拼出 magnet，不查 DHT、不取元数据 |
| `providers` | 列出已配置的源（不显示 URL 和密钥），不发搜索请求 |

## 3. 搜索基础

```bash
# 默认源，按 seeders 降序，显示前 20 条
magnet search "ubuntu"

# 指定源（逗号分隔或重复 --source）
magnet search "ubuntu" --source knaben --json
magnet search "ubuntu" --source nyaa,knaben --jsonl

# 只要 magnet 列表，方便喂给脚本
magnet search "ubuntu" --magnet
```

没有配置时，默认并发查询 Nyaa、Knaben、Sukebei、Bitsearch 和 BtGoogle。APIBay、DMHY、Anime Garden、Mikan 需用 `--source` 显式启用。各内置源的差异见 [Provider 配置](providers.zh-CN.md)。

## 4. 过滤、排序与数量

```bash
magnet search "ubuntu" \
  --min-seeds 10 --min-size 1G --max-size 10G \
  --sort seeds --limit 20
```

- `--sort seeds|size|date|title`：前三种降序、未知值排最后，标题升序。
- 大小支持 `10G`、`1.5GiB` 或直接写字节数。
- 设置 seeds 或大小过滤后，对应字段未知的结果会被排除。
- 顺序是：**先按 BTIH 去重 → 再过滤 → 再排序 → 最后 `--limit`**。

## 5. 输出格式

`--json`、`--jsonl`、`--magnet` 三者互斥；都不选时输出表格。

- `--json`：一个 JSON 数组。
- `--jsonl`：每行一个 JSON 对象，聚合完成后一次性输出。
- `--magnet`：每行一个 magnet，跳过无法构造 magnet 的结果。

表格中未知的 seeders、大小和日期显示为 `?`，大小以十进制 GB 显示，AGE 以天计。JSON 字段含义见 [脚本与集成](scripting.zh-CN.md)。

## 6. 全局参数与环境变量

| 参数 | 环境变量 | 作用 |
| --- | --- | --- |
| `--config PATH` | `MAGNET_CONFIG` | 指定 TOML 配置文件 |
| `--cache PATH` | `MAGNET_CACHE` | 指定搜索快照文件，便于隔离会话 |

命令行参数优先于对应环境变量，全局参数可以放在子命令前后。程序不会自动读取当前目录的 `config.toml`，需要显式指定路径或设置 `MAGNET_CONFIG`。

## 7. 搜索快照与 `get`

搜索结束会把结果写入快照（Linux 默认 `~/.cache/magnet/last-search.json`）。`get ID` 只读快照、不联网，ID 仅在最近一次搜索内有效。

```bash
magnet get 1          # 输出第 1 条的 magnet
magnet get 1 --json   # 输出第 1 条的完整 JSON 对象

# 会话隔离（脚本或 agent 常用）
export MAGNET_CACHE="$PWD/session-search.json"
magnet search "ubuntu" --json
magnet get 1 --json
```

更多细节（快照替换规则、无 magnet 记录）见 [脚本与集成](scripting.zh-CN.md)。

## 8. `resolve`

```bash
magnet resolve 0000000000000000000000000000000000000000 --name "Example" --json
```

接受 40 位十六进制或 32 位 base32 BTIH，可重复传入 `--tracker URL`。它只校验 hash 并构造 magnet，不查询 DHT、不解析详情页、不获取 torrent 元数据；全零 hash 仅用于演示。

## 9. 选取某条记录的磁链

三种常用做法，取决于你手里有什么定位方式。

**方式一：快照 ID + `get`（最贴合 CLI）**

结果中每条记录都有从 1 开始的 `id`，就是表格或 JSON 里显示的那个：

```bash
magnet search "ubuntu"   # 先看结果，记下想要的那条 ID
magnet get 3             # 输出第 3 条的 magnet
magnet get 3 --json      # 输出第 3 条的完整 JSON
```

ID 只在最近一次搜索内有效，重新搜索会重新编号。该记录没有 magnet 时，`get ID` 退出码为 `1`；`get ID --json` 仍能读到完整记录。

**方式二：`--json` + `jq` 按条件挑（最灵活）**

```bash
# 按位置（第 1 条，即默认 seeders 最高的那条）
magnet search "ubuntu" --json | jq -r '.[0].magnet'

# 按 id 字段
magnet search "ubuntu" --json | jq -r '.[] | select(.id==3) | .magnet'

# 按标题关键字
magnet search "ubuntu" --json | jq -r '.[] | select(.title | test("22.04")) | .magnet'

# 手动取 seeders 最高的一条（对 null 兜底）
magnet search "ubuntu" --json | jq -r 'sort_by(-(.seeders // 0))[0].magnet'
```

**方式三：`--magnet` 取第 N 行**

```bash
magnet search "ubuntu" --magnet | sed -n '3p'
```

**脚本里更稳妥的写法**：先确认搜索成功且目标记录确有合法磁链，再继续使用。

```bash
if results=$(magnet search "ubuntu" --json) &&
   uri=$(printf '%s' "$results" | jq -er '.[0].magnet | select(type == "string" and startswith("magnet:?"))'); then
    echo "$uri"   # 这里换成你的客户端，例如 pikpaktui offline "$uri"
fi
```

一句话总结：**要呈现给人看再选就用 `get ID`；要在脚本里按条件自动选就用 `--json` + `jq`。**

## 10. 配置自定义源

```bash
cp config.example.toml config.toml
magnet --config config.toml providers --json
magnet --config config.toml search "ubuntu" --json
```

显式配置会**替换**默认源列表，而不是追加。每个 `[[providers]]` 需要 `name`、`kind` 和 HTTP(S) `url`；`default_search = false` 可让某源只在 `--source` 时启用。完整字段、各 `kind` 的行为，以及 Torznab / RSS 示例见 [Provider 配置](providers.zh-CN.md)。

## 11. 退出码与脚本集成

stdout 放结果，stderr 放警告和错误，脚本要分别捕获并检查退出码：

| 码 | 含义 |
| --- | --- |
| `0` | 有结果，或其他命令成功 |
| `1` | 无结果、快照中没有该 ID，或该结果没有可输出的 magnet |
| `2` | 全部源失败（含解析/运行配置错误），或本地 I/O 失败 |
| `3` | 无效参数或配置，如未知 source、配置不可读 |
| `4` | 全部 Provider 因 HTTP/网络/超时失败 |

完整的 Python/Bash 示例与 pikpaktui 集成见 [脚本与集成](scripting.zh-CN.md)。

## 12. 技巧与限制

- 标题过滤：默认丢弃标题未包含**全部**查询词（忽略大小写）的结果，每次丢弃都会在 stderr 打印。查询写法与标题不一致时（如 `C++`）可用 `--no-title-filter`。
- CJK 查询：公网源容易返回兜底热榜，建议叠加 Knaben 或 Bitsearch；APIBay 对 CJK 已从默认搜索中封存。
- 默认并发 8 个源、单源超时 15 秒。用 `--concurrency`（1–64）、`--timeout`（1–300）、`--deadline`（总时限 1–3600）和 `--pages`（1–20）调节。
- BtGoogle 靠抓取 HTML，页面结构变化时最易失效；本项目不含下载器、TUI、DHT 查询或 MCP。

## 相关文档

- [Provider 配置](providers.zh-CN.md)
- [脚本与集成](scripting.zh-CN.md)
- [文档索引](README_CN.md)
