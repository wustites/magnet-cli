# bitmagnet 集成

[bitmagnet](https://github.com/bitmagnet-io/bitmagnet) 是自托管的 BitTorrent 索引器和 DHT crawler。它与 Prowlarr 聚合外部 Indexer 的工作方式不同：bitmagnet 会持续从 DHT 发现资源，并将结果保存到本地 PostgreSQL 数据库。

## 当前部署

部署目录：`/opt/bitmagnet`

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

## Caddy 公网查询入口

`https://bm.20070809.xyz/` 由 Caddy 反向代理到本机 `127.0.0.1:3333`。访问根路径时 bitmagnet 会自动跳转到 `/webui`，Web UI 的静态资源和查询请求都通过同一域名访问。

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

## 当前数据量

截至 2026-09-15 08:14 UTC：

| 项目 | 数量 |
| --- | ---: |
| Torrent | 2,291 |
| 文件记录 | 29,044 |
| 内容实体 | 115 |
| 待处理队列任务 | 18 |
| 已处理队列任务 | 9 |
| 数据库大小 | 27 MB |

针对哈希 `A02AFF57F86A48407A57E17DBEA6FC09C9150570` 和固定测试词 `子子西` 查询，目前均为 0 条。

## Prowlarr 集成

bitmagnet 暴露 Torznab 接口，可以在 Prowlarr 中添加 **Generic Torznab**：

```text
Name:     Bitmagnet DHT
URL:      http://127.0.0.1:3333
API Path: /torznab/api
API Key:  留空
```

当前 bitmagnet 刚启动，Prowlarr 的 Indexer 测试因查询没有结果而暂未保存。待数据库出现可搜索结果后，再从 Prowlarr 添加并测试。

官方文档：[Installation](https://bitmagnet.io/setup/installation.html)、[Endpoints](https://bitmagnet.io/guides/endpoints.html)、[Servarr Integration](https://bitmagnet.io/guides/servarr-integration.html)。
