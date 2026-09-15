# bitmagnet 集成

[bitmagnet](https://github.com/bitmagnet-io/bitmagnet) 是自托管的 BitTorrent 索引器和 DHT crawler。它与 Prowlarr 聚合外部 Indexer 的工作方式不同：bitmagnet 会持续从 DHT 发现资源，并将结果保存到本地 PostgreSQL 数据库。

## 当前部署

部署目录：`/opt/bitmagnet`

```text
Web UI:       http://127.0.0.1:3333/webui/
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
