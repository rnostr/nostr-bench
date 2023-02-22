use clap::Parser;
#[macro_use]
extern crate clap;

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
    Connect {
        /// Nostr relay host url
        #[arg(value_name = "URL")]
        url: String,

        /// Max count of clients
        #[arg(short = 'n', long, default_value = "100", value_name = "NUM")]
        count: usize,

        /// Connection rate every second
        #[arg(short = 'r', long, default_value = "50", value_name = "NUM")]
        rate: usize,

        /// Set the amount of threads
        #[arg(short = 't', long, default_value = "1", value_name = "NUM")]
        threads: usize,

        /// Interface address
        #[arg(long)]
        ifaddr: Option<String>,
    },
}

fn main() {
    let args = Cli::parse();
    match args.command {
        Commands::Connect {
            url,
            count,
            rate,
            threads,
            ifaddr,
        } => {
            println!("Connect {url}");
        }
    }
}
