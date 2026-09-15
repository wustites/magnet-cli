# Prowlarr 集成

Prowlarr 是索引器管理与聚合服务，不是单独的 BT 搜索站点。本项目通过 Prowlarr 的 Torznab API 接入多个 Indexer。

## 当前实例

- 本地管理地址：`http://127.0.0.1:9696`
- Caddy 地址：`https://prowlarr.20070809.xyz`
- Caddy 上游：`127.0.0.1:9696`
- 已配置 Indexer：`BigFANGroup`、`Knaben`、`U3C3`
- 外部访问受 Caddy Basic Auth 保护

公共 Indexer 通常不需要账户；私有 Indexer 可能需要账户、邀请码、Cookie 或站点 API Key。

## 获取 API Key

在 Prowlarr 中打开：

```text
Settings → General → Security → API Key
```

API Key 只用于 `magnet-cli` 访问 Prowlarr，不等于 Indexer 的账户凭据。不要将真实 Key 写入配置文件或提交到 Git。

## magnet-cli 配置

推荐在同一台机器上使用本地地址：

```toml
[[providers]]
name = "prowlarr"
kind = "torznab"
url = "http://127.0.0.1:9696/3/api"
api_key_env = "PROWLARR_API_KEY"
default_search = true
```

运行：

```bash
export PROWLARR_API_KEY='从 Prowlarr 复制的 Key'
magnet search '子子西' --source prowlarr --json
```

Caddy 的完整 Torznab 地址是 `https://prowlarr.20070809.xyz/3/api`；但当前 Caddy 入口有 Basic Auth，而 `magnet-cli` 的 Torznab Provider 只发送 API Key，因此优先使用本地地址。

## 当前测试

使用固定回归词 `子子西` 测试时，Prowlarr API 返回 HTTP 200，共 85 条结果，全部来自 `Knaben`；标题中完整包含 `子子西` 的结果为 0 条。`BigFANGroup` 和 `U3C3` 本次没有返回结果。
