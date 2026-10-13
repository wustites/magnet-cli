# Provider 配置

[English](providers.md)

magnet 内置 9 个 Provider，并支持自定义 Torznab 与 RSS/Atom。本页是完整参考；快速上手见 [使用教程](usage.zh-CN.md)，总览见 [README_CN](../README_CN.md)。

## 默认与显式配置

没有指定配置时，默认并发查询 Nyaa RSS、Knaben JSON、Sukebei RSS、Bitsearch 和 BtGoogle。显式配置会**替换默认 Provider 列表**，不会追加到默认列表。内置共 9 个 Provider，其中这 5 个参与默认搜索，APIBay、DMHY、Anime Garden 和 Mikan 需通过 `--source` 按需启用。

从 [config.example.toml](../config.example.toml) 开始：

```bash
cp config.example.toml config.toml
magnet --config config.toml providers --json
magnet --config config.toml search "ubuntu" --json
```

每个 `[[providers]]` 条目必须包含 `name`、`kind` 和 HTTP(S) `url`。`name` 必须非空且唯一，只能使用 ASCII 字母、数字、连字符或下划线。至少配置一个源；未知配置字段会被拒绝。`api_key_env` 仅适用于 Torznab 和 Bitsearch，且必须是合法的环境变量名。`default_search` 默认为 `true`，因此不希望参与默认聚合搜索的源必须显式写出 `default_search = false`。

## `kind` 一览

| `kind` | 请求方式 | 搜索行为 |
| --- | --- | --- |
| `nyaa` | HTTP GET RSS | 添加 `page=rss` 和搜索参数 `q` |
| `sukebei` | HTTP GET RSS | 使用 Nyaa RSS 协议，添加 `page=rss` 和 `q` |
| `knaben` | HTTP POST JSON | 向配置的 API URL 提交标题搜索，单次请求 150 条 |
| `apibay` | HTTP GET JSON | 在 APIBay 的全部分类中搜索；读取 API 固定返回的结果集。查询无法匹配时 APIBay 会返回兜底结果，由通用标题过滤剔除 |
| `bitsearch` | HTTP GET JSON | 搜索 Bitsearch，每页 100 条；可选 API key 请求头 |
| `dmhy` | HTTP GET RSS | 使用 DMHY 关键词 RSS 搜索；通过 `--source dmhy` 启用 |
| `animegarden` | HTTP POST JSON | 使用 Anime Garden 全文动漫搜索；通过 `--source animegarden` 启用 |
| `mikan` | HTTP GET RSS | 使用蜜柑计划关键词 RSS 搜索；通过 `--source mikan` 启用 |
| `btgoogle` | HTTP GET HTML | 使用 BtGoogle 搜索结果页，默认参与聚合搜索 |
| `torznab` | HTTP GET RSS/XML | 添加 `t=search`、`q`、`extended=1`，可带 API key |
| `rss` | HTTP GET RSS/Atom | 原样请求 URL，在本地按标题过滤；搜索词按空白拆分，忽略大小写且每个词都必须匹配 |

## 分页

`--pages` 分别使用 Knaben 的 `from` 偏移、Bitsearch 的 `page` 和 Torznab 的 `offset`/`limit`。APIBay、Nyaa/Sukebei RSS 与通用 RSS/Atom 没有可靠的兼容分页机制，因此只请求一次。Provider 返回空页或末页时提前停止；单 Provider 超时包含其请求的所有页面。

## 内置源说明

- **APIBay**：对“苏畅”等非 ASCII/CJK 查询有时会返回热门兜底结果，而不是匹配项。CLI 会要求返回标题包含每个查询词，并在本地移除这些无关结果。由于 CJK 覆盖不稳定，APIBay 已从默认聚合搜索中封存，需要时仍可通过 `--source apibay` 显式测试。
- **DMHY**：可选搜索源，使用 `--source dmhy` 启用，调用 DMHY 公开的关键词 RSS 接口，默认聚合搜索中不会自动启用。
- **Anime Garden**：可选搜索源，使用 `--source animegarden` 启用，调用项目提供的公开 JSON API，默认聚合搜索中不会自动启用。
- **蜜柑计划（Mikan Project）**：可选搜索源，使用 `--source mikan` 启用，调用站点的关键词 RSS 搜索接口，默认聚合搜索中不会自动启用。
- **BtGoogle**：调用站点的搜索结果接口，默认参与聚合搜索，也可以通过 `--source btgoogle` 单独查询。
- **Bitsearch**：匿名额度目前为每个 IP 每日 200 次。需要使用账号 API key 时，在该 Provider 上设置 `api_key_env`；密钥通过 `x-api-key` 请求头发送，不会出现在诊断信息中。

搜索 CJK 内容时，建议同时启用 Knaben 或 Bitsearch，以获得更好的覆盖率。

## 搜索测试词

[testdata/search-keywords.csv](../testdata/search-keywords.csv) 提供可复用的搜索测试词清单。第一项 `子子西` 是固定 CJK 回归词；测试多个源时应保留该词不变。CSV 的 `category`、`keyword` 和 `purpose` 列分别表示类别、搜索词和测试目的。

例如只取固定回归词：

```bash
awk -F, 'NR > 1 { print $2; exit }' testdata/search-keywords.csv
magnet search '子子西' --source btgoogle --json
```

## Sukebei：Nyaa NSFW

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

所有 Provider 都支持可选字段 `default_search`（省略时为 `true`）。设为 `false` 时仍会出现在 `providers` 列表中，但仅在 `--source` 显式选择时参与搜索。内置 Sukebei 和示例配置将其设为 `true`，参与默认聚合搜索；改为 `false` 可排除默认搜索。若没有可用于默认搜索的源，须传入 `--source`，否则返回退出码 3。

## Torznab：Jackett / Prowlarr

Prowlarr 的配置、Caddy 反代、API Key 和当前测试状态见 [Prowlarr 集成文档](prowlarr.md)。本项目建议在同一台机器上使用 `http://127.0.0.1:9696/3/api`。

[bitmagnet 集成文档](bitmagnet.md)和 [bitmagnet 数据备份与恢复](bitmagnet-backup.md)记录的是早期开发阶段使用的自建 DHT 索引部署。两份文档已于 2026-09-21 **封存**，不再维护，仅保留作参考。

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

## RSS

```toml
[[providers]]
name = "linux"
kind = "rss"
url = "https://example.org/torrents.rss"
```

上面的 URL 是占位示例，需替换为实际 RSS 或 Atom feed。解析器支持 RSS item 和 Atom entry 中的 magnet 链接、enclosure，以及 Nyaa/Torznab 扩展字段。仅有 `.torrent` 下载链接时，不会自动下载文件计算 hash。普通 Newznab NZB 结果不能转换成 BitTorrent magnet。

## 协议参考

- [Knaben API](https://knaben.org/api/v1/)
- [Bitsearch API](https://bitsearch.eu/api)
- [Nyaa RSS 模板](https://github.com/nyaadevs/nyaa/blob/master/nyaa/templates/rss.xml)
- [Torznab 规范](https://torznab.github.io/spec-1.3-draft/torznab/Specification-v1.3.html)
