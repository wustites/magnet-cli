# bitmagnet 集成

> **已封存（2026-09-21）**：本机 bitmagnet 部署已停用，仓库不再维护这部分集成。本文档保留作历史参考，其中的接口行为、实测数据与检索特性描述仍然有效。
>
> 若日后重新部署，索引需要从零开始积累——DHT 抓取无法复现原有 210 万条记录，入库速率随去重而下降，且内容不会一致。

[bitmagnet](https://github.com/bitmagnet-io/bitmagnet) 是自托管的 BitTorrent 索引器和 DHT crawler。它与 Prowlarr 聚合外部 Indexer 的工作方式不同：bitmagnet 会持续从 DHT 发现资源，并将结果保存到本地 PostgreSQL 数据库。

## 部署结构（已封存）

封存前的部署目录：`/opt/bitmagnet`

```text
Web UI:       http://127.0.0.1:3333/webui/
公网 Web UI:  https://bm.20070809.xyz/
Torznab API:  http://127.0.0.1:3333/torznab/api
BitTorrent:   TCP/UDP 3334
数据库:       /opt/bitmagnet/data/postgres
配置:         /opt/bitmagnet/config
```

服务由 Docker Compose 管理：

```bash
cd /opt/bitmagnet
sudo docker compose ps
sudo docker compose logs -f bitmagnet
sudo docker compose restart bitmagnet
```

健康检查：

```bash
curl http://127.0.0.1:3333/status
```

数据库、配置的备份策略及完整恢复步骤见[数据备份与恢复](bitmagnet-backup.md)。

## Caddy 公网查询入口（已封存）

封存前，`https://bm.20070809.xyz/` 由 Caddy 反向代理到本机 `127.0.0.1:3333`。访问根路径时 bitmagnet 会自动跳转到 `/webui`，Web UI 的静态资源和查询请求都通过同一域名访问。

入口沿用其他子站点的 `admin` Basic Auth，并启用 HTTPS、gzip/zstd 压缩和基础安全响应头。未认证请求返回 `401`。由于认证覆盖整个站点，公网 Torznab 地址 `https://bm.20070809.xyz/torznab/api` 也需要 Basic Auth；`magnet-cli` 应继续使用本机地址 `http://127.0.0.1:3333/torznab/api`。

Caddy 上游只监听回环地址，bitmagnet 的 DHT TCP/UDP `3334` 端口不经过 Caddy。

## 查询

通过 Torznab API 查询关键词：

```bash
curl -sG 'http://127.0.0.1:3333/torznab/api' \
  --data-urlencode 't=search' \
  --data-urlencode 'q=A02AFF57F86A48407A57E17DBEA6FC09C9150570'
```

也可以打开 Web UI 进行搜索。bitmagnet 的索引需要持续运行后逐步积累，刚启动时查询结果为空是正常现象；目标资源还必须能从 DHT 获得元数据或 peer 信息。

## magnet-cli 搜索

bitmagnet 的 Torznab 接口不需要 API Key，直接配成 `torznab` provider 即可：

```toml
[[providers]]
name = "bitmagnet"
kind = "torznab"
url = "http://127.0.0.1:3333/torznab/api"
default_search = true
```

```bash
magnet search '流浪地球' --config ~/.config/magnet-cli/bitmagnet.toml --limit 20
```

注意 `--config` 会替换内置 Provider 列表；若要与 Nyaa、Knaben 等一起搜索，需要在同一份配置里补上这些条目。

### 关键词列表回归

用项目自带的 `testdata/search-keywords.csv` 逐词回归（无结果时 `magnet` 返回退出码 1）：

```bash
awk -F, 'NR > 1 { gsub(/"/, "", $2); print $2 }' testdata/search-keywords.csv \
  | xargs -d '\n' -I{} sh -c \
      'printf "%s\t" "{}"; magnet search "{}" --config ~/.config/magnet-cli/bitmagnet.toml --limit 100 --json | jq length'
```

不要用 `while read -r kw; do ... magnet search "$kw" ...; done` 这种写法：在某些环境里 `read` 与 `printf` 的组合会把 UTF-8 字节按 Latin-1 重新编码（`子子西` 的 `E5 AD 90` 变成 `C3 A5 C2 AD C2 90`），CJK 关键词全部变成垃圾查询，而垃圾查询照样返回 100 条，于是回归看起来「全部通过」，实际把 `可笑的单纯` 这类真实 0 结果掩盖掉。用 `od -c` 对比传递前后的字节可以立刻验证：`xargs -d '\n'` 或 Python 脚本都保持原字节。

2026-09-21 09:20 UTC 实测（库内 210 万 torrent，crawler 运行 6 天）：

| 类别 | 关键词 | 结果数 | 标题字面包含 | 最高做种 | 首条标题 |
| --- | --- | ---: | ---: | ---: | --- |
| cjk_exact | `子子西` | 7 | 2 | 0 | 840175573247937698.cc】IPZ338隔壁的色娘鄰居姊姊~希志あいの |
| cjk_name | `苏畅` | 100 | 77 | 3 | www.98T.la@苏畅作品 四部 在沙发上疯狂做爱，差点被18公分巨屌玩坏了！ |
| cjk_phrase | `可笑的单纯` | 0 | — | — | 无结果 |
| cjk_phrase | `无理的前进` | 0 | — | — | 无结果 |
| cjk_traditional | `繁體中文` | 100 | 7 | 5 | 策略游戏：三国志 12 威力加强版安装繁体中文版San12PK |
| cjk_japanese | `東京` | 100 | 27 | 3 | JAV(無碼、無修正、UNCENSORED) 東京熱 东京热 Tokyo Hot n0001-0100 (2003-20 |
| english | `ubuntu` | 100 | 71 | 321 | ubuntu-25.10-live-server-amd64.iso |
| numeric | `1080p` | 100 | 73 | 126 | Fullmetal Alchemist Brotherhood S01 (BD 1080p HEVC Opus) [Du |
| person | `Skylar Vox` | 100 | 16 | 9 | ManyVids.23.12.20.Skylar.Vox.Big.Titty.PornStar.Skylar.Vox.F |
| person | `kiva` | 41 | 1 | 23 | [OZC-Live]Kamen Rider Den-O BD Box Complete Series [720p] |
| person | `mspuiyi` | 6 | 3 | 2 | VA - Music News vol.391 |
| catalog | `jufe-016` | 12 | 1 | 0 | 04月12日-有碼高清中文字幕-一百二十部合集 |
| cjk_name | `陈妮妮` | 12 | 1 | 3 | Hyoyeon (김효연) x Kim Gap-ju (김갑주) x Ming Sun Ha (하밍선) x Hanso |

13 个词里 11 个有结果（`可笑的单纯`、`无理的前进` 为 0，`magnet` 返回退出码 1）。`100` 表示触达单次查询上限，实际匹配更多——单个关键词的结果数随 crawler 持续增长，上表只是某一时刻的快照。

Torznab 单次查询的 `limit` 上限确实是 100（传 `limit=200` 仍只返回 100 条，缺省也是 100），但支持 `offset` 翻页，每页最多 100 条，因此 `magnet` 的 `--limit 100` 不是覆盖上限。要越过 top-100 需要绕开 `magnet-cli` 直接分页查询，例如确认某个低做种资源是否仍被索引：

```bash
for off in 0 100 200 300 400 500; do
  curl -sG 'http://127.0.0.1:3333/torznab/api' \
    --data-urlencode 't=search' --data-urlencode 'q=東京' \
    --data-urlencode 'limit=100' --data-urlencode "offset=$off" \
    | grep -q 'Tokyo.Drifter' && { echo "offset=$off 命中"; break; }
done
# offset=500 命中
```

排序按做种数降序，低做种的长尾资源会被挤出第一页，做回归时不要把「top-100 里没有」直接当成「索引里没有」。

### 命中不等于字面包含

bitmagnet 的 Torznab `q` 走全文检索，不是子串匹配，`magnet-cli` 也不会对 torznab 结果做本地标题过滤。索引（`torrent_contents.tsv`）由种子名、**文件路径**和关联的元数据（TMDb 等）构建，规则：

- 非 ASCII 字符按 unidecode 转写成拼音并逐字成词：`繁體中文` → `'Fan' <-> 'Ti' <-> 'Zhong' <-> 'Wen'`，因此繁体与简体互通，`繁體中文` 会命中简体标题《三国群英传7绿色免安装繁体中文版》。
- 标点分词后用 AND 连接：`jufe-016` → `'jufe' & '016'`，命中文件路径为 `JUFE-569-uncensored-.mp4` 和 `GENU-016-uncensored-.mp4` 的种子（`5d73fd23f3071e3c186683e3f0d98094ca2a696`，种子名是「12月22日-精选高清无码（破坏版）一百四十四合集」，两个文件名都不出现在种子名里）。
- 文件名参与检索：`陈妮妮` 在 12 条结果里有 11 条的种子名不含该词，这些种子是靠文件列表命中的，例如种子 `More girls photos 更多更好的寫真資源。【TG：@vv9900pp】/【微密圈】精选系列02。鱼神+黑饱宝+葱油饼er+妮是老虎-陈妮妮UNI。網紅VIP私密寫真。共289套.torrent` 出现在某韩国 OnlyFans 合集的文件路径里。
- 元数据参与检索：`東京` → `'Dong' <-> 'Jing'` 命中 `Tokyo.Drifter.1966.Criterion.1080p.BluRay.x265.HEVC.AAC-SARTRE`，靠的是该影片的原始标题；该条目做种数低，在 09-21 已落到 top-100 之外，需要 `offset=500` 才能取到（见上节的分页脚本）。

需要严格字面匹配时自行过滤：

```bash
magnet search '繁體中文' --config ~/.config/magnet-cli/bitmagnet.toml --json --limit 100 \
  | jq '[.[] | select(.title | contains("繁體中文"))] | length'   # 100 条结果中字面命中 7 条
```

字面命中率随索引增长而提高，早期（09-16，库内 33 万）`繁體中文` 是 0/5、`陈妮妮` 是 0/1、`jufe-016` 是 0/1；09-21（库内 210 万）三者分别为 7/100、1/12、1/12，即「命中但不含关键词」仍普遍存在（`陈妮妮` 12 条里 11 条属于这种情况），但已不是绝对规律。CJK 多字短语要求整串拼音相邻，命中难度最高：`可笑的单纯`、`无理的前进` 至今均为 0 条。

示例输出（`magnet search 流浪地球`）：

```text
 ID  SEED      SIZE       AGE      SOURCE          NAME
  1  38        1.46 GB    5d       bitmagnet       The.Wandering.Earth.2019.DUBBED.1080p.WEBRip.1400MB.DD5.1.x264-GalaxyRG[TGx]
  2  30        2.55 GB    3d       bitmagnet       The.Wandering.Earth.II.2023.1080p.Chinese.WEB-DL.HC.H264.AAC-HHWEB.mkv
  3  22        4.47 GB    6d       bitmagnet       The Wandering Earth 2 2023 1080p (Dual) BluRay HEVC x265 5.1 BONE.mkv
```

结果数随 crawler 运行持续增长，同一关键词在几分钟内即可能增加，且做种数也在变（`流浪地球` 首条 09-16 为 22 做种，09-21 为 38 做种）。

## 封存时的数据量

截至 2026-09-21 06:40 UTC（DHT crawler 连续运行 5 天 22 小时）：

| 项目 | 数量 |
| --- | ---: |
| Torrent | 2,103,763 |
| 文件记录（torrent_files） | 28,113,361 |
| 内容实体（content） | 75,175 |
| 内容属性（content_attributes） | 227,549 |
| 关联记录（torrent_contents） | 2,103,445 |
| 合集（content_collections） | 4,517 |
| 待处理队列任务 | 5 |
| 已处理队列任务 | 25,474 |
| 数据库大小 | 16.0 GB = 14.9 GiB（`pg_database_size`；磁盘占用 16 GB，另含 480 MB WAL） |

各表占用（`pg_total_relation_size`，含索引与 TOAST）：

| 表 | 合计 | 堆 | 索引 + TOAST |
| --- | ---: | ---: | ---: |
| torrent_files | 10 GB | 3.7 GB | 6.4 GB |
| torrent_contents | 3.1 GB | 1.2 GB | 1.9 GB |
| torrents | 755 MB | 320 MB | 434 MB |
| torrents_torrent_sources | 603 MB | 211 MB | 392 MB |
| queue_jobs | 327 MB | 9.7 MB | 317 MB |
| content | 66 MB | 44 MB | 22 MB |

`torrent_files` 是最大表，占库容三分之二。它只为有文件列表的种子写入行：28,113,361 行分布在 1,379,387 个 info_hash 上（= `multi` 1,271,056 + `over_threshold` 108,331），平均 20.4 行/种子；`single` 种子（725,151）不产生文件记录。该表索引约为堆的 1.75 倍，主要来自 `torrent_files_pkey` 3.97 GB 和 `torrent_files_info_hash_index_key` 1.9 GB。

内容实体按类型分布：`movie` 56,854、`tv_show` 14,983、`xxx` 3,346。

入库速率：过去 24 小时新增 299,024 torrent（约 1.25 万/小时），全期均值约 1.48 万/小时（35 万/天），速率随 DHT 去重而缓降。队列已基本消化，无积压。

容量：约 7.1 KB/torrent，库容自 09-16 起以约 2.4 GB/天增长（2.34 GB → 16.0 GB，129 小时）。宿主机 `/dev/sda1` 总 192.7 GiB、已用 96.1 GiB、剩余 96.6 GiB（49.9%，`duf` 读数）——当时按 2.4 GB/天估算约 40 天见底，这正是封存该部署的原因之一。索引膨胀（`torrent_files` 索引 6.4 GB）在删除历史数据后不会自动回落，需 `REINDEX`/`VACUUM FULL` 才能回收；相关流程见[数据备份与恢复](bitmagnet-backup.md)。

封存时搜索可用，各关键词实测数量见上节。哈希 `A02AFF57F86A48407A57E17DBEA6FC09C9150570` 一直为 0 条——该资源未被 DHT 发现，与索引规模无关。

## Prowlarr 集成（已封存）

bitmagnet 暴露 Torznab 接口，封存前可以在 Prowlarr 中添加 **Generic Torznab**：

```text
Name:     Bitmagnet DHT
URL:      http://127.0.0.1:3333
API Path: /torznab/api
API Key:  留空
```

该接口在封存时已能返回结果（见上节），但对应的 Indexer 已随部署一并停用。

官方文档：[Installation](https://bitmagnet.io/setup/installation.html)、[Endpoints](https://bitmagnet.io/guides/endpoints.html)、[Servarr Integration](https://bitmagnet.io/guides/servarr-integration.html)。
