# bitmagnet 数据备份与恢复

> **已封存（2026-09-21）**：本文档随本机 bitmagnet 部署一并停用。备份与恢复流程、实测数据与陷阱说明仍然有效，可作为同类 PostgreSQL 部署的参考。
>
> 文中引用的 `/opt/bitmagnet`、`bitmagnet-postgres`、`/var/backups/bitmagnet` 等路径属于该已封存的部署，均已删除，使用前请先确认这些路径的当前状态。
>
> 保留下来的那份逻辑备份（2026-09-21T07:47:48Z）已移至 PikPak 云盘：`~/pik/Backup/bitmagnet/20260921T074748Z/`，含 `bitmagnet.dump`、`config.tar.gz`、`database-info.txt`、`SHA256SUMS`。该副本已按本文「验证备份」流程校验（`sha256sum --check SHA256SUMS` 全部 OK），并与删除前的本地文件逐字节一致。这是当前唯一副本；恢复时建议先 `rclone copy` 回本地磁盘再 `pg_restore`，云盘挂载的随机读很慢。

本文适用于项目当前的 Docker Compose 部署：bitmagnet 位于 `/opt/bitmagnet`，PostgreSQL 16 容器名为 `bitmagnet-postgres`，数据库名为 `bitmagnet`，超级用户名为 `postgres`。**不存在名为 `bitmagnet` 的角色**，`pg_dump -U bitmagnet` 会以 `FATAL: role "bitmagnet" does not exist` 失败——数据库名和角色名在这里不是一回事。

bitmagnet 的主要持久化数据分为两部分：

- PostgreSQL 数据库：torrent、文件、内容元数据和处理队列。
- 部署配置：`docker-compose.yml` 和 `config/`。当前 `config/` 是空目录，连接参数由 Compose 的环境变量（`POSTGRES_HOST`、`POSTGRES_PASSWORD`、`POSTGRES_DB`）注入，`sudo docker exec bitmagnet bitmagnet config show` 会显示这些值来源为 `env`。因此配置备份的实际对象是 `docker-compose.yml`。

数据库不能靠重新爬 DHT 恢复：DHT 发现结果具有随机性，且按当前约 1.25 万 torrent/小时的速度，现有的 210 万条记录需要连续爬取约 6 天才能回到同一量级，内容也不会一致。备份是唯一可靠的恢复手段。

### 为什么 16 GB 的库只有 1.5 GB 备份

不算异常。`pg_dump` 导出的是逻辑内容，三部分不进 dump：

| 库内构成 | 大小 | 是否进逻辑 dump |
| --- | ---: | --- |
| 索引（`pg_indexes_size`） | 9.6 GB | 否，只导出 `CREATE INDEX` 语句，恢复时重建 |
| 表数据 heap + TOAST（`pg_table_size`） | 6.3 GB | 是 |
| WAL | 544 MB | 否 |
| `pg_database_size` 合计 | 16 GB | — |

实测未压缩逻辑 dump（`pg_dump -Fp` 纯 SQL）为 7,405,114,012 字节 = 6.9 GiB，与 6.3 GB 行数据吻合；`-Fc --compress=6` 再压缩 4.7 倍得到 1.48 GiB。

压缩率高来自数据本身的高度冗余：`torrent_files` 单表 3780 MB 中 `path` 列占 1433 MB，2811 万条路径共享目录前缀与命名模式。该表单独测：未压缩 4,870,901,189 字节 → zstd 后 576,085,324 字节，**8.5 倍**。

推论：逻辑恢复慢在索引重建而非数据导入（9.6 GB 索引要现建），这也是物理备份恢复只要 17 s 的原因。

## 两种备份方式

| 方式 | 输出 | 备份耗时 | 恢复耗时 |
| --- | ---: | ---: | ---: |
| 逻辑备份 `pg_dump -Fc --compress=6` | 1.48 GiB | 239 s | 23 min |
| 逻辑备份 `pg_dump -Fd -j 4 --compress=zstd:6` | 1.27 GiB | 114 s | 未单独实测 |
| 物理备份 `pg_basebackup -Fp -Xs` | 15.4 GiB | 499 s | 17 s |

实测条件：2026-09-21，数据库 16.0 GB（torrent 2,103,763；torrent_files 28,113,361 行；content 75,175），PostgreSQL 16.15 aarch64。逻辑备份的恢复耗时指在全新的 `postgres:16-alpine` 容器中执行 `pg_restore -j 4` 直到索引与约束全部建完；物理备份的恢复耗时指从挂载数据目录到 `database system is ready to accept connections`。逻辑恢复耗时对磁盘竞争很敏感：空载 1356 s，与 `pg_basebackup` 同时跑时为 2359 s。`-Fd` 的恢复未单独计时，同为 `-j 4` 且主要耗时在索引与约束构建，预期与 `-Fc` 相近。

选择方式：

- 当前规模（16 GB）：逻辑备份够用，日常首选 `-Fd -j 4 --compress=zstd:6`，比 `-Fc --compress=6` 小 16%、快一倍。
- 迁移、跨 PostgreSQL 版本、需要单表恢复（`pg_restore -t`）：逻辑备份。
- 灾难恢复、要求分钟级恢复：物理备份，但体积是逻辑备份的约 10 倍，且只能原样恢复到兼容版本的 PostgreSQL。
- 两者不互斥。建议每日逻辑备份，每周或每次升级前追加一次物理备份。

## 逻辑备份

`pg_dump` 生成一致性快照，不需要停止 crawler。备份期间数据库继续写入，因此备份只包含 `pg_dump` 开始时已提交的数据。

### 创建

custom 格式（单文件，便于复制到对象存储）：

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

directory 格式，多 worker 并行，速度约翻倍、体积约小 16%（PostgreSQL 16 支持 `--compress=zstd:N`，镜像内含 zstd 1.5.7；省略 `--compress` 时的默认算法是 gzip，产物为 `.dat.gz`）：

```bash
sudo docker exec bitmagnet-postgres sh -c \
  'rm -rf /tmp/bitmagnet-backup && \
   pg_dump -U postgres -d bitmagnet --format=directory -j 4 --compress=zstd:6 \
     -f /tmp/bitmagnet-backup'
sudo docker exec bitmagnet-postgres pg_restore --list /tmp/bitmagnet-backup >/dev/null
sudo docker cp bitmagnet-postgres:/tmp/bitmagnet-backup "$backup_dir/dir"
sudo docker exec bitmagnet-postgres rm -rf /tmp/bitmagnet-backup
```

实测输出 20 个文件，其中最大的两个是 `torrent_files`（552 MB）和 `torrent_contents`（539 MB），其余在 139 MB 以下。目录先落在容器的 `/tmp`，而容器可写层在宿主机根分区上，因此这段临时占用计入 `/dev/sda1`：`-Fd` 流程在 `docker cp` 完成前会同时占用容器内和宿主机两份空间（约 2.6 GiB），custom 格式用 `| sudo tee` 则只占宿主机一份（约 1.5 GiB）。

`pipefail` 可防止 `pg_dump` 失败后仍把不完整文件当成成功备份；若命令中途失败，应删除该时间戳目录后重新执行。

### 恢复

恢复会覆盖目标数据库中的现有数据。先确认备份路径和校验结果正确；如果现有数据库仍有保留价值，应先再做一次备份。

`-j` 与标准输入不兼容，`pg_restore -j 4` 读管道会报 `parallel restore from standard input is not supported`。single-file 的 custom dump 需要先复制进容器（或者把备份目录挂载进容器）再恢复：

```bash
cd /opt/bitmagnet
sudo docker compose stop bitmagnet

backup_dir=/var/backups/bitmagnet/20260921T074748Z
sudo sh -c "cd '$backup_dir' && sha256sum --check SHA256SUMS"

sudo docker exec bitmagnet-postgres \
  psql -U postgres -d postgres -v ON_ERROR_STOP=1 -c \
  "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = 'bitmagnet' AND pid <> pg_backend_pid();"

sudo docker exec bitmagnet-postgres dropdb -U postgres --if-exists bitmagnet
sudo docker exec bitmagnet-postgres createdb -U postgres bitmagnet

sudo docker cp "$backup_dir/bitmagnet.dump" bitmagnet-postgres:/tmp/bitmagnet.dump
sudo docker exec bitmagnet-postgres \
  pg_restore -U postgres -d bitmagnet --no-owner --exit-on-error /tmp/bitmagnet.dump
sudo docker exec bitmagnet-postgres rm -f /tmp/bitmagnet.dump
```

这里用 `dropdb`/`createdb` 重建数据库，而不是 `pg_restore --clean --if-exists`：`--clean` 只能删除备份里存在的对象，备份之后新建的表、扩展会残留下来，恢复结果并不是备份时的状态。

directory 格式同上，只是把 `docker cp` 的目标换成目录，并把 `-j 4` 加到 `pg_restore`：

```bash
sudo docker cp "$backup_dir/dir" bitmagnet-postgres:/tmp/bitmagnet-backup
sudo docker exec bitmagnet-postgres \
  pg_restore -U postgres -d bitmagnet --no-owner --exit-on-error -j 4 /tmp/bitmagnet-backup
```

`--exit-on-error` 让第一个失败就终止恢复，而不是留下一个缺索引、缺外键的半成品库。默认模式虽然会在结尾以非零码退出（`warning: errors ignored on restore: N`），但出错之前已执行的对象不会回滚，脚本配合 `set -e` 也来不及阻止半成品状态。恢复完成后启动服务并检查：

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

## 物理备份

物理备份复制整个数据目录，恢复时不需要回放 SQL，因此恢复时间与库容基本无关。运行中的集群**不能**用 `tar`/`rsync` 直接复制数据目录：文件复制无法保证数据文件、WAL 和 checkpoint 之间的一致性，得到的副本可能无法启动或静默损坏。`pg_basebackup` 是 PostgreSQL 官方的在线物理备份工具，它自己处理 checkpoint 和 WAL。

### 创建

```bash
set -euo pipefail
umask 077
sudo install -d -m 0700 /var/backups/bitmagnet
backup_dir="/var/backups/bitmagnet/$(date -u +%Y%m%dT%H%M%SZ)-phys"
sudo install -d -m 0700 "$backup_dir"

sudo docker exec bitmagnet-postgres sh -c \
  'rm -rf /tmp/pgdata-backup && \
   pg_basebackup -U postgres -h /var/run/postgresql -D /tmp/pgdata-backup \
     -Fp -Xs -P --checkpoint=fast'
sudo docker exec bitmagnet-postgres pg_verifybackup /tmp/pgdata-backup
sudo docker cp bitmagnet-postgres:/tmp/pgdata-backup "$backup_dir/pgdata"
sudo docker exec bitmagnet-postgres rm -rf /tmp/pgdata-backup
```

要点：

- `postgres` 角色在本部署中已是超级用户且具备 `rolreplication`，`listen_addresses = '*'`，回环地址为 `trust`，因此不需要单独的复制用户或密码；`-h localhost` 同样可用。
- `-Fp` 输出是可直接启动的数据目录，`-Xs` 把备份所需的 WAL 一起写入。`--checkpoint=fast` 让主库立即做一次 checkpoint，缩短备份时间。
- 实测 15,768,274 kB（15.4 GiB）/ 499 s，`pg_verifybackup` 349 s。
- `docker cp` 把 15.4 GiB 移出容器用了约 12 分钟（707 s，含随后的容器内清理），是整条链路里最慢的一步；直接挂载宿主机目录给容器写入会更快。
- 镜像内自带 `pg_basebackup`、`pg_verifybackup` 和 `zstd`，无需额外安装。若要压缩归档，用 tar 格式加 `-Z zstd:6`（`--compress` 的形式是 `[client|server-]METHOD[:DETAIL]`，写裸数字表示 gzip 级别），而不是 `-Fp`。

### 从物理备份恢复

物理备份的恢复就是让一个 PostgreSQL 进程使用这份数据目录：

```bash
sudo chown -R 70:70 "$backup_dir/pgdata"   # alpine 镜像里 postgres 的 uid 是 70

# 旧容器还在时先改名或删除，避免 --name 冲突
sudo docker rm -f bitmagnet-postgres

sudo docker run -d --name bitmagnet-postgres \
  -v "$backup_dir/pgdata":/pgdata -e PGDATA=/pgdata \
  --restart unless-stopped \
  postgres:16-alpine

sudo docker logs bitmagnet-postgres
sudo docker exec bitmagnet-postgres \
  psql -U postgres -d bitmagnet -c "SELECT count(*) FROM torrents;"
```

`docker cp` 出的数据目录会保留备份时的集群状态，因此这一步之后仍要确认 bitmagnet 服务连得上（`POSTGRES_HOST` 等变量由 Compose 提供，见部署文档）。换一台主机恢复时还要确认 Compose 的挂载路径、端口、防火墙和凭据。

要点：

- `docker cp` 出来的文件属主是 root，必须先 `chown` 成容器内 `postgres` 的 uid（alpine 镜像为 70），否则启动失败。
- **`POSTGRES_PASSWORD` 在此无效**：官方 entrypoint 只在 `PGDATA` 为空时运行 initdb 并写入该口令，已存在的集群保留原口令。恢复后的集群凭据与备份来源一致。
- 实测：挂载后 17 s 内 `database system is ready to accept connections`；bitmagnet 对其启动时 `goose: no migrations to run. current version: 20`，Torznab 查询正常返回。
- 只能恢复到兼容的 PostgreSQL 主版本和架构（本部署为 `postgres:16-alpine` / aarch64）；跨主版本需要 `pg_upgrade`。
- 恢复后的容器仍要按部署文档补上端口映射和 bitmagnet 服务的连接参数；上例只起了数据库本身。

## 备份中的陷阱

1. **不要给 `docker exec` 加 `-t`**。`docker exec -t ... pg_dump -Fc > dump` 这种写法会让伪终端把 `\n` 翻译成 `\r\n`，静默损坏二进制 dump。实测连续 3 次 `-t` 备份全部损坏（每次多出 65,794 字节的 `\r`），`pg_restore --list` 报 `could not read from input file: end of file`；不加 `-t` 的 3 次全部正常。结论：从容器往外取二进制只用 `docker exec`（重定向到宿主机文件），往容器里送二进制用 `docker exec -i`，两者都不要 `-t`。
2. **`-Fc` 不支持并行**：`pg_dump -Fc -j 4` 直接报 `parallel backup only supported by the directory format`。
3. **`pg_restore -j` 不支持标准输入**，见上文；先落盘再并行恢复。
4. **`-Fd -j` 不会切分单表**。并行度作用在表之间，单张表始终由一个 worker 处理。实测 `pg_dump -Fd -j 4 -t torrents` 只产生一个数据文件（当时 161 MB）。当前库最大的表 `torrent_files` 压缩后 552 MB、`torrent_contents` 539 MB，两张大表可以并行，`-j 4` 仍有意义；但如果将来 80% 的容量集中在单张表上（bitmagnet 长期运行后 `torrent_files` 最容易变成这种表），`-j` 对那张表没有帮助，此时物理备份才是有意义的选项。
5. **不要对已压缩的 dump 再压缩**。对 1.48 GiB 的 custom dump 跑 `zstd -6`，得到 1,586,135,493 字节，比原文件还大 11 KB。要更小就在 `pg_dump` 里换 `--compress=zstd:6`。
6. **磁盘余量**：逻辑备份占宿主机约 1.3–1.5 GiB，物理备份约 15.4 GiB，且容器内 `/tmp` 与宿主机根分区共用空间。库容当前以约 2.4 GB/天增长，宿主机根分区 192.7 GiB、剩余 96.6 GiB（49.9%，`duf` 读数），按此速率约 40 天见底，需要在到达之前决定清理还是扩容。注意物理备份本身要占 15.4 GiB，若同时保留多份并与增长叠加，见底时间会明显提前。

## 验证备份

每次备份后至少验证校验和，以及 dump 是否可解析：

```bash
backup_dir=/var/backups/bitmagnet/20260921T074748Z
sudo sh -c "cd '$backup_dir' && sha256sum --check SHA256SUMS"
sudo cat "$backup_dir/bitmagnet.dump" \
  | sudo docker exec -i bitmagnet-postgres pg_restore --list >/dev/null
```

directory 格式的 `pg_restore --list` 需要目录本身可见，因此在容器内创建完、`docker cp` 之前就校验（见上文创建命令）。这两项只能发现文件结构损坏，不能代替恢复演练。逻辑备份实测恢复耗时约 23 分钟（`-j 4`），物理备份 17 秒，所以演练成本可接受时优先演练物理备份。

## 恢复演练

在一次性容器里恢复，不影响生产实例。逻辑备份演练：

```bash
sudo docker run -d --name pg-rehearsal \
  -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=bitmagnet postgres:16-alpine
sleep 10

sudo docker cp "$backup_dir/bitmagnet.dump" pg-rehearsal:/tmp/bitmagnet.dump
sudo docker exec pg-rehearsal \
  pg_restore -U postgres -d bitmagnet --no-owner --exit-on-error -j 4 /tmp/bitmagnet.dump

sudo docker exec pg-rehearsal psql -U postgres -d bitmagnet -c \
  "SELECT (SELECT count(*) FROM torrents) AS torrents, \
          (SELECT count(*) FROM torrent_files) AS files, \
          (SELECT count(*) FROM torrent_contents) AS torrent_contents, \
          (SELECT count(*) FROM content) AS content;"
sudo docker exec pg-rehearsal psql -U postgres -d bitmagnet -Atc \
  "SELECT count(*) FILTER (WHERE NOT indisvalid) FROM pg_index; \
   SELECT count(*) FROM pg_constraint WHERE contype = 'f'; \
   SELECT max(version_id) FROM goose_db_version;"
```

后一条查询应当返回：无效索引 0、外键 19、迁移版本 20。可以用完好的 bitmagnet 镜像直接对着演练库起一个只读 HTTP worker，验证数据真的能被应用读取：

```bash
sudo docker network create bm-rehearsal
sudo docker network connect bm-rehearsal pg-rehearsal
sudo docker run -d --name bm-rehearsal --network bm-rehearsal \
  -p 127.0.0.1:13333:3333 \
  -e POSTGRES_HOST=pg-rehearsal -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=bitmagnet \
  ghcr.io/bitmagnet-io/bitmagnet:latest worker run --keys=http_server
sleep 20
curl -s http://127.0.0.1:13333/status
curl -sG 'http://127.0.0.1:13333/torznab/api' --data-urlencode 't=search' --data-urlencode 'q=ubuntu' \
  | grep -c '<item>'
```

清理：`sudo docker rm -f -v bm-rehearsal pg-rehearsal && sudo docker network rm bm-rehearsal`。物理备份演练同理，用 `-v "$backup_dir/pgdata":/pgdata -e PGDATA=/pgdata` 起容器，跳过 `pg_restore`。

**必须带 `-v`**：`postgres` 镜像在 Dockerfile 里声明了 `VOLUME /var/lib/postgresql/data`，`docker run` 会给演练容器创建匿名卷。只执行 `docker rm -f` 时容器消失但匿名卷留下（实测 4 个演练卷合计 29 GB），`docker rm -v` 才会一起删除；也可以一开始就用 `--rm`。

## 定期备份与清理

将上文命令整理成 root 管理的脚本后，可通过 cron 或 systemd timer 每日运行。脚本应启用 `set -euo pipefail`，只有在备份、归档和校验全部成功后才清理旧备份。备份增长很快，保留策略建议按份数而不是天数：逻辑备份保留 14 份、物理备份保留 2–3 份，并确认副本已复制到异机或对象存储。

例如，确认备份已经复制到异机后，删除 14 天以前的本机备份目录：

```bash
sudo find /var/backups/bitmagnet -mindepth 1 -maxdepth 1 \
  -type d -mtime +14 -print
```

先检查输出，再将末尾的 `-print` 改为 `-exec rm -rf -- {} +`。不要对变量为空或未经核对的路径执行递归删除。

建议监控以下项目：

- 最近一次成功备份的时间、大小和校验结果；
- 备份磁盘的剩余空间，以及库容增长速度（当前约 2.4 GB/天、剩余约 40 天，另需扣掉物理备份占用的 15.4 GiB）；
- dump 中记录的 torrent 数量是否异常下降；
- 异机副本是否存在；
- 最近一次完整恢复演练的日期与结果。

## 冷备份说明

只有在 PostgreSQL 完全停止后，才能直接归档 `/opt/bitmagnet/data/postgres`；运行中复制该目录得到的副本不保证一致性，不要把它当作备份。冷备份通常只能恢复到兼容的 PostgreSQL 版本和架构，体积也大（当前 16 GB），因此只在必须整机搬迁时使用：先 `sudo docker compose down`，确认两个容器均已停止，再复制整个数据目录；复制完成后立即重新启动服务。日常的物理备份应当使用 `pg_basebackup`，它可以在服务运行期间完成。

## 数据规模与清理

数据库当前以约 2.4 GB/天增长，`torrent_files` 一张表就占 10 GB（库容 16 GB 的三分之二），且其索引（6.4 GB）大于堆本身。删除历史数据时注意两点：

- `torrent_files` 的索引不会随 `DELETE` 自动回落，需要 `VACUUM FULL` 或 `REINDEX` 才能把空间还给操作系统；
- 只有空间真正回收之后，备份体积才会下降，所以清理完成后再做一次备份，而不是指望 `DELETE` 立刻缩小备份。

封存前的部署未配置归档/时间线相关清理，`archive_mode = off`、`max_wal_senders = 10`、`pg_wal` 占用约 480 MB，属于正常水平。
