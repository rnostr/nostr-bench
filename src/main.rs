use clap::Parser;
#[macro_use]
extern crate clap;

mod connect;
mod runtime;

/// Nostr relay benchmarker

#[derive(Debug, Parser)] // requires `derive` feature
#[command(
    name = "nostr-bench",
    about = "Nostr relay benchmarking tool.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Connection benchmark
    #[command(arg_required_else_help = true)]
    Connect(connect::ConnectOpts),
}

fn main() {
    let args = Cli::parse();
    match args.command {
        Commands::Connect(opts) => {
            let rt = runtime::get_rt(opts.threads);
            rt.block_on(connect::start(opts.clone()));
        }
    }
}
