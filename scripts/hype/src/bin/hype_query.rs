//! `hype_query` — CLI to query a CA's hype score.
//!
//! Usage:
//!   hype_query --ca <CONTRACT_ADDRESS>
//!
//! Returns cached score (if fresh). In scaffold mode, prints "no data" when
//! there is no cached entry — live fetch is wired up in phase 3.

use clap::Parser;

use hype::get_hype_score;

#[derive(Parser, Debug)]
#[command(name = "hype_query", about = "Query the hype score for a Solana CA")]
struct Args {
    /// Contract address (mint) to query.
    #[arg(long)]
    ca: String,

    /// Override DB path (defaults to data/hype.db).
    #[arg(long)]
    db: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Some(db) = args.db {
        std::env::set_var("HYPE_DB_PATH", db);
    }

    match get_hype_score(&args.ca).await {
        Ok(h) => {
            // Pretty JSON so the bot can pipe it.
            match serde_json::to_string_pretty(&h) {
                Ok(s) => println!("{}", s),
                Err(e) => {
                    eprintln!("error: failed to serialize HypeScore: {}", e);
                    std::process::exit(2);
                }
            }
        }
        Err(e) => {
            // Scaffold: surface "no data" cleanly when nothing cached + no live fetch.
            eprintln!("no data: {}", e);
            std::process::exit(1);
        }
    }
}
