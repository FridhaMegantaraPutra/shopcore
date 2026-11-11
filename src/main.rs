//! CLI entry point: `serve` runs the HTTP API, the other subcommands are the
//! maintenance commands (equivalent of legacy `manage.py`).

use std::net::SocketAddr;

use chrono::{NaiveDate, Utc};
use clap::{Parser, Subcommand};
use shopcore::application::{maintenance, reports};
use shopcore::infrastructure::db::connect_with_retry;
use shopcore::interfaces::http::{router, AppState};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "shopcore", about = "ShopCore order, wallet and reporting service")]
struct Cli {
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply migrations, then start the HTTP server.
    Serve {
        #[arg(long, env = "BIND_ADDR", default_value = "0.0.0.0:8000")]
        bind: SocketAddr,
        #[arg(long, env = "BANK_WEBHOOK_SECRET")]
        webhook_secret: String,
    },
    /// Apply pending database migrations.
    Migrate,
    /// Insert demo wallets.
    Seed,
    /// Rebuild wallet balances from the transaction ledger.
    RecomputeBalances,
    /// Print the daily report as JSON (default: today, UTC).
    Report {
        #[arg(long)]
        date: Option<NaiveDate>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let cli = Cli::parse();
    let db = connect_with_retry(&cli.database_url, 30).await?;

    match cli.command {
        Command::Serve { bind, webhook_secret } => {
            maintenance::migrate(&db).await?;
            let app = router(AppState { db, webhook_secret });
            let listener = tokio::net::TcpListener::bind(bind).await?;
            tracing::info!(%bind, "listening");
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await?;
        }
        Command::Migrate => {
            maintenance::migrate(&db).await?;
            println!("migrations applied");
        }
        Command::Seed => {
            maintenance::migrate(&db).await?;
            maintenance::seed(&db).await?;
            println!("seeded");
        }
        Command::RecomputeBalances => {
            let fixed = maintenance::recompute_balances(&db).await?;
            println!("recomputed balances, {fixed} wallet(s) corrected");
        }
        Command::Report { date } => {
            let date = date.unwrap_or_else(|| Utc::now().date_naive());
            let r = reports::daily_report(&db, date).await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
        }
    }
    Ok(())
}
