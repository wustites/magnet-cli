use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "magnet",
    version,
    about = "Concurrent torrent search and magnet resolver"
)]
pub struct Cli {
    #[arg(long, global = true, env = "MAGNET_CONFIG")]
    pub config: Option<PathBuf>,
    #[arg(long, global = true, env = "MAGNET_CACHE")]
    pub cache: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    Search(SearchArgs),
    /// Read a one-based ID from the last search snapshot
    Get {
        #[arg(value_parser = clap::value_parser!(u64).range(1..))]
        id: u64,
        #[arg(long)]
        json: bool,
    },
    /// Construct a magnet locally; does not query DHT or retrieve metadata
    Resolve {
        infohash: String,
        #[arg(long, default_value = "")]
        name: String,
        #[arg(long)]
        tracker: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// List configured providers without revealing URLs or credentials
    Providers {
        #[arg(long)]
        json: bool,
    },
}
#[derive(Args)]
#[command(group(clap::ArgGroup::new("format").args(["json", "jsonl", "magnet"])))]
pub struct SearchArgs {
    pub query: String,
    #[arg(long, value_delimiter = ',')]
    pub source: Vec<String>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub jsonl: bool,
    #[arg(long)]
    pub magnet: bool,
    #[arg(long)]
    pub min_seeds: Option<u32>,
    #[arg(long, value_parser = crate::model::parse_size)]
    pub min_size: Option<u64>,
    #[arg(long, value_parser = crate::model::parse_size)]
    pub max_size: Option<u64>,
    #[arg(long, value_enum, default_value = "seeds")]
    pub sort: Sort,
    #[arg(long, default_value = "20", value_parser = clap::value_parser!(u32).range(1..))]
    pub limit: u32,
    #[arg(long, default_value = "15", value_parser = clap::value_parser!(u64).range(1..=300))]
    pub timeout: u64,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Sort {
    Seeds,
    Size,
    Date,
    Title,
}
