# bitmagnet 数据备份与恢复

本文适用于项目当前的 Docker Compose 部署：bitmagnet 位于 `/opt/bitmagnet`，PostgreSQL 16 容器名为 `bitmagnet-postgres`，数据库名为 `bitmagnet`。

bitmagnet 的主要持久化数据分为两部分：

- PostgreSQL 数据库：torrent、文件、内容元数据和处理队列。应使用 `pg_dump` 备份，不要在数据库运行时直接复制 `/opt/bitmagnet/data/postgres`。
- 部署配置：`docker-compose.yml` 和 `config/`。其中可能包含密码或 API Key，备份文件应限制访问权限。

## 备份策略

建议每天执行一次逻辑备份，至少保留 7 天，并定期把备份复制到另一台主机或对象存储。只有保存在同一块磁盘上的副本不能防范磁盘故障。

`pg_dump` 会生成数据库的一致性快照，不需要停止 crawler。备份期间数据库仍会持续写入，因此备份只包含命令开始时已经提交的数据。

## 创建备份

安装目录和备份目录可以按实际环境调整。以下命令创建 PostgreSQL 自定义格式备份、配置归档、校验文件和记录清单：

```bash
set -euo pipefail
umask 077

sudo install -d -m 0700 /var/backups/bitmagnet

backup_time=$(date -u +%Y%m%dT%H%M%SZ)
backup_dir="/var/backups/bitmagnet/$backup_time"
sudo install -d -m 0700 "$backup_dir"

sudo docker exec bitmagnet-postgres \
  pg_dump -U postgres -d bitmagnet --format=custom --compress=6 \
  | sudo tee "$backup_dir/bitmagnet.dump" >/dev/null

sudo tar -C /opt/bitmagnet -czf "$backup_dir/config.tar.gz" \
  docker-compose.yml config

sudo docker exec bitmagnet-postgres \
  psql -U postgres -d bitmagnet -Atc \
  "SELECT current_timestamp, pg_database_size(current_database()), count(*) FROM torrents;" \
  | sudo tee "$backup_dir/database-info.txt" >/dev/null

sudo sh -c "cd '$backup_dir' && \
  sha256sum bitmagnet.dump config.tar.gz database-info.txt > SHA256SUMS && \
  chmod 0600 ./*"
```

自定义格式通常比纯 SQL 更小，并支持 `pg_restore` 校验和并行恢复。`pipefail` 可防止 `pg_dump` 失败后仍把不完整文件当成成功备份；若命令中途失败，应删除该时间戳目录后重新执行。

查看备份大小：

```bash
sudo du -sh "$backup_dir"
sudo cat "$backup_dir/database-info.txt"
```

## 验证备份

每次备份后至少验证校验和以及 dump 目录是否可读：

```bash
backup_dir=/var/backups/bitmagnet/20260918T030000Z
sudo sh -c "cd '$backup_dir' && sha256sum --check SHA256SUMS"
sudo cat "$backup_dir/bitmagnet.dump" \
  | sudo docker exec -i bitmagnet-postgres pg_restore --list >/dev/null
```

`pg_restore --list` 只能发现文件结构损坏，不能代替恢复演练。建议定期在独立 PostgreSQL 实例中完整恢复，并查询关键表数量。

## 恢复数据库

恢复会删除目标 `bitmagnet` 数据库中的现有数据。先确认备份路径和校验结果正确；如果现有数据库仍有保留价值，应先再做一次备份。

停止 bitmagnet 以避免恢复期间继续写入，但保持 PostgreSQL 运行：

```bash
cd /opt/bitmagnet
sudo docker compose stop bitmagnet

backup_dir=/var/backups/bitmagnet/20260918T030000Z
sudo sh -c "cd '$backup_dir' && sha256sum --check SHA256SUMS"
```

终止连接、重建数据库并恢复：

```bash
sudo docker exec bitmagnet-postgres \
  psql -U postgres -d postgres -v ON_ERROR_STOP=1 -c \
  "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = 'bitmagnet' AND pid <> pg_backend_pid();"

sudo docker exec bitmagnet-postgres \
  dropdb -U postgres --if-exists bitmagnet
sudo docker exec bitmagnet-postgres \
  createdb -U postgres bitmagnet

sudo cat "$backup_dir/bitmagnet.dump" \
  | sudo docker exec -i bitmagnet-postgres \
  pg_restore -U postgres -d bitmagnet --exit-on-error --no-owner
```

恢复配置时先检查归档内容，避免误覆盖当前配置：

```bash
sudo tar -tzf "$backup_dir/config.tar.gz"
sudo tar -C /opt/bitmagnet -xzf "$backup_dir/config.tar.gz"
```

启动服务并检查数据库、HTTP 状态和日志：

```bash
cd /opt/bitmagnet
sudo docker compose up -d
sudo docker compose ps

sudo docker exec bitmagnet-postgres \
  psql -U postgres -d bitmagnet -c \
  "SELECT count(*) AS torrents FROM torrents;"
curl --fail http://127.0.0.1:3333/status
sudo docker compose logs --tail=100 bitmagnet
```

如果恢复到不同版本的 PostgreSQL，目标主版本应不低于备份来源版本。恢复到另一台主机时，还需确认 Compose 挂载路径、端口、防火墙和配置中的凭据。

## 定期备份与清理

将“创建备份”命令整理成 root 管理的脚本后，可通过 cron 或 systemd timer 每日运行。脚本应启用 `set -euo pipefail`，只有在 `pg_dump`、归档和校验全部成功后才清理旧备份。

例如，确认备份已经复制到异机后，删除 14 天以前的本机备份目录：

```bash
sudo find /var/backups/bitmagnet -mindepth 1 -maxdepth 1 \
  -type d -mtime +14 -print
```

先检查输出，再将末尾的 `-print` 改为 `-exec rm -rf -- {} +`。不要对变量为空或未经核对的路径执行递归删除。

建议监控以下项目：

- 最近一次成功备份的时间、大小和校验结果；
- 备份磁盘的剩余空间；
- dump 中记录的 torrent 数量是否异常下降；
- 异机副本是否存在；
- 最近一次完整恢复演练的日期与结果。

## 冷备份说明

只有在 PostgreSQL 完全停止后，才可以直接归档 `/opt/bitmagnet/data/postgres`。这种物理副本通常只能恢复到兼容的 PostgreSQL 版本和架构，体积也较大，因此不作为默认方案。若必须使用，先执行 `sudo docker compose down`，确认两个容器均已停止，再复制整个数据目录；复制完成后立即重新启动服务。
