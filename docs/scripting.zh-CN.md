# 脚本与集成

[English](scripting.md)

本页汇总 JSON 结果结构、聚合规则、搜索快照、退出码，以及脚本和 pikpaktui 集成方式。快速上手见 [使用教程](usage.zh-CN.md)，总览见 [README_CN](../README_CN.md)。

## JSON 结果结构

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

`get --json` 和 `resolve --json` 输出单个对象；`providers --json` 输出包含 `name`、`kind`、`default_search` 的数组。

## 聚合规则

聚合以 BTIH 为主键，不使用标题。base32 hash 会转换成十六进制；没有 hash 的结果即使同名也不会合并。同一 hash 的 sources、trackers 取并集，seeders、leechers 取最大报告值，不累加。

标题采用配置顺序中最先出现的记录；大小、发布时间和详情链接优先保留该记录的已有值，缺失时从后续记录补齐。Provider 完成请求的先后顺序不影响这个优先级。无效或冲突的 hash 会被跳过并输出警告。

重建 magnet 时保留 BTIH、显示名称和 tracker，其他 magnet 参数不保留。当前支持 BTIH/v1，不支持仅包含 v2 hash 的 magnet。

## 标题过滤

公网索引会把 CJK 查询按单字切分；当查询没有真实匹配时，它们会用最热门的结果填满整页。因此搜索会丢弃标题未包含查询词（按空白切分、忽略大小写、需全部命中）的结果。该过滤按 Provider 执行，在 magnet 规范化之后、去重之前，并且**对所有查询生效**，不限于非 ASCII 查询。每次丢弃都会在 stderr 输出 `NAME: dropped N result(s) that do not match the query`，过滤绝不会静默发生。

匹配采用朴素的子串判断。它刻意保持可预测而非聪明，代价是当查询写法与标题不一致时可能被误杀：搜索 `C++` 会丢弃 `Microsoft Visual C+ Redistributable`，但保留 `Visual C++ [2005-2015]`。当查询的写法在标题中不会原样出现时，用 `--no-title-filter` 关闭过滤。

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

## 退出码

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

## Python 示例

下面的示例无需额外依赖，并区分无结果与执行失败：

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

## 选取某条记录的磁链

- 呈现给人看再选：用快照 ID，`magnet get ID`（无 magnet 时退出 1）。
- 在脚本里按条件自动选：用 `--json` + `jq`。

```bash
# 位置
magnet search "ubuntu" --json | jq -r '.[0].magnet'
# id 字段
magnet search "ubuntu" --json | jq -r '.[] | select(.id==3) | .magnet'
# 标题关键字
magnet search "ubuntu" --json | jq -r '.[] | select(.title | test("22.04")) | .magnet'
# seeders 最高（对 null 兜底）
magnet search "ubuntu" --json | jq -r 'sort_by(-(.seeders // 0))[0].magnet'
# 第 N 行 magnet
magnet search "ubuntu" --magnet | sed -n '3p'
```

更稳健的取用方式（先确认成功且磁链合法）：

```bash
if results=$(magnet search "ubuntu" --json) &&
   uri=$(printf '%s' "$results" | jq -er '.[0].magnet | select(type == "string" and startswith("magnet:?"))'); then
    echo "$uri"
fi
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
