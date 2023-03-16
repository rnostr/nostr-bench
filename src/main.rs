//! Nostr relay benchmarker
use clap::Parser;
#[macro_use]
extern crate clap;
use nostr_bench::{connect, event, req, runtime};

/// Cli
#[derive(Debug, Parser)]
#[command(
    name = "nostr-bench",
    about = "Nostr relay benchmarking tool.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Commands
#[derive(Debug, Subcommand)]
enum Commands {
    /// Connection benchmark
    #[command(arg_required_else_help = true)]
    Connect(connect::ConnectOpts),
    /// Publish event benchmark
    #[command(arg_required_else_help = true)]
    Event(event::EventOpts),
    /// Request event benchmark
    #[command(arg_required_else_help = true)]
    Req(req::ReqOpts),
}

fn main() {
    let args = Cli::parse();
    println!("{:?}", args);
    match args.command {
        Commands::Connect(opts) => {
            let rt = runtime::get_rt(opts.threads);
            rt.block_on(connect::start(opts.clone()));
        }
        Commands::Event(opts) => {
            let rt = runtime::get_rt(opts.threads);
            rt.block_on(event::start(opts.clone()));
        }
        Commands::Req(opts) => {
            let rt = runtime::get_rt(opts.threads);
            rt.block_on(req::start(opts.clone()));
        }
    }
}
