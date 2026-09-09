use anyhow::{Context, Result};
use clap::Parser;
use magnet_cli::{
    cache,
    cli::{Cli, Command, Sort},
    model::{Torrent, normalize_hash},
    providers::{self, HttpProvider, Provider},
    search,
};
use std::{
    io::{self, Write},
    process::ExitCode,
    time::Duration,
};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 3 } else { 0 };
            let _ = error.print();
            return ExitCode::from(code);
        }
    };
    match run(cli).await {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
            {
                return ExitCode::SUCCESS;
            }
            eprintln!("magnet: {error:#}");
            ExitCode::from(2)
        }
    }
}
fn invalid(message: &str) -> Result<u8> {
    eprintln!("magnet: {message}");
    Ok(3)
}
fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

async fn run(cli: Cli) -> Result<u8> {
    let path = cli.cache.unwrap_or_else(cache::default_path);
    let mut out = io::BufWriter::new(io::stdout());
    match cli.command {
        Command::Resolve {
            infohash,
            name,
            tracker,
            json,
        } => {
            let hash = match normalize_hash(&infohash) {
                Ok(hash) => hash,
                Err(e) => return invalid(&e.to_string()),
            };
            let mut row = Torrent {
                title: name,
                info_hash: Some(hash),
                trackers: tracker,
                ..Default::default()
            };
            row.normalize()?;
            if json {
                serde_json::to_writer(&mut out, &row)?;
                writeln!(out)?;
            } else {
                writeln!(out, "{}", row.magnet.unwrap())?;
            }
        }
        Command::Get { id, json } => {
            let rows = cache::read(&path)?;
            let Some(row) = rows.iter().find(|r| r.id as u64 == id) else {
                eprintln!("magnet: ID not found in last search");
                return Ok(1);
            };
            if json {
                serde_json::to_writer(&mut out, row)?;
                writeln!(out)?;
            } else if let Some(magnet) = &row.magnet {
                writeln!(out, "{magnet}")?;
            } else {
                eprintln!("magnet: result has no BTIH/magnet; inspect its detail_url with --json");
                return Ok(1);
            }
        }
        Command::Providers { json } => {
            let configs = match providers::configuration(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => return invalid(&e.to_string()),
            };
            let rows: Vec<_> = configs
                .iter()
                .map(|c| serde_json::json!({"name": c.name, "kind": c.kind}))
                .collect();
            if json {
                serde_json::to_writer(&mut out, &rows)?;
                writeln!(out)?;
            } else {
                for row in rows {
                    writeln!(
                        out,
                        "{}\t{}",
                        row["name"].as_str().unwrap(),
                        row["kind"].as_str().unwrap()
                    )?;
                }
            }
        }
        Command::Search(args) => {
            if args.query.trim().is_empty() {
                return invalid("query must not be empty");
            }
            if args
                .min_size
                .zip(args.max_size)
                .is_some_and(|(min, max)| min > max)
            {
                return invalid("min-size exceeds max-size");
            }
            let mut configs = match providers::configuration(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => return invalid(&e.to_string()),
            };
            for source in &args.source {
                if !configs.iter().any(|c| &c.name == source) {
                    return invalid(&format!("unknown source: {source}"));
                }
            }
            configs.retain(|c| args.source.is_empty() || args.source.contains(&c.name));
            let timeout = Duration::from_secs(args.timeout);
            let client = reqwest::Client::builder()
                .timeout(timeout)
                .connect_timeout(timeout.min(Duration::from_secs(10)))
                .user_agent(concat!("magnet-cli/", env!("CARGO_PKG_VERSION")))
                .build()?;
            let providers: Vec<Box<dyn Provider>> = configs
                .into_iter()
                .map(|config| {
                    Box::new(HttpProvider {
                        config,
                        client: client.clone(),
                    }) as Box<dyn Provider>
                })
                .collect();
            let mut report = search::search(&providers, &args.query, timeout).await;
            for warning in &report.warnings {
                eprintln!("magnet: {warning}");
            }
            report.results.retain(|r| {
                args.min_seeds
                    .is_none_or(|min| r.seeders.is_some_and(|n| n >= min))
                    && args
                        .min_size
                        .is_none_or(|min| r.size.is_some_and(|n| n >= min))
                    && args
                        .max_size
                        .is_none_or(|max| r.size.is_some_and(|n| n <= max))
                    && (!args.magnet || r.magnet.is_some())
            });
            report.results.sort_by(|a, b| {
                let ordering = match args.sort {
                    Sort::Seeds => b.seeders.cmp(&a.seeders),
                    Sort::Size => b.size.cmp(&a.size),
                    Sort::Date => b.published_at.cmp(&a.published_at),
                    Sort::Title => a.title.cmp(&b.title),
                };
                ordering
                    .then_with(|| a.title.cmp(&b.title))
                    .then_with(|| a.info_hash.cmp(&b.info_hash))
            });
            report.results.truncate(args.limit as usize);
            for (i, row) in report.results.iter_mut().enumerate() {
                row.id = i + 1;
            }
            cache::save(&path, &report.results).context("cannot save search snapshot")?;
            if args.json {
                serde_json::to_writer(&mut out, &report.results)?;
                writeln!(out)?;
            } else if args.jsonl {
                for row in &report.results {
                    serde_json::to_writer(&mut out, row)?;
                    writeln!(out)?;
                }
            } else if args.magnet {
                for row in &report.results {
                    writeln!(out, "{}", row.magnet.as_ref().unwrap())?;
                }
            } else {
                writeln!(
                    out,
                    " ID  SEED      SIZE       AGE      SOURCE          NAME"
                )?;
                for row in &report.results {
                    let age = row
                        .published_at
                        .map(|d| format!("{}d", (chrono::Utc::now() - d).num_days().max(0)))
                        .unwrap_or_else(|| "?".into());
                    writeln!(
                        out,
                        "{:>3}  {:<8}  {:<9}  {:<7}  {:<14}  {}",
                        row.id,
                        row.seeders
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "?".into()),
                        row.size
                            .map(|n| format!("{:.2} GB", n as f64 / 1e9))
                            .unwrap_or_else(|| "?".into()),
                        age,
                        row.sources.join(","),
                        clean(&row.title)
                    )?;
                }
            }
            out.flush()?;
            return Ok(if report.successes == 0 {
                if report.network_only { 4 } else { 2 }
            } else if report.results.is_empty() {
                1
            } else {
                0
            });
        }
    }
    out.flush()?;
    Ok(0)
}
