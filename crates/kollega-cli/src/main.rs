//! Binaire `kollega` : serve, migrate, version.

#![forbid(unsafe_code)]

use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kollega", version, about = "Runtime d'agents IA gouvernés")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Démarre le serveur HTTP.
    Serve {
        /// Adresse d'écoute.
        #[arg(long, env = "KOLLEGA_LISTEN", default_value = "127.0.0.1:8080")]
        listen: String,
    },
    /// Applique les migrations à la base de données.
    Migrate,
    /// Affiche la version du binaire.
    Version,
}

fn database_url() -> anyhow::Result<String> {
    std::env::var("DATABASE_URL").context("la variable d'environnement DATABASE_URL est absente")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match Cli::parse().command {
        Command::Serve { listen } => {
            let pool = kollega_store::connect(&database_url()?)
                .await
                .context("connexion à PostgreSQL impossible")?;
            let app = kollega_api::router(pool);
            let listener = tokio::net::TcpListener::bind(&listen)
                .await
                .with_context(|| format!("impossible d'écouter sur {listen}"))?;
            tracing::info!(%listen, "kollega démarre");
            axum::serve(listener, app)
                .await
                .context("le serveur HTTP s'est arrêté en erreur")?;
        }
        Command::Migrate => {
            let pool = kollega_store::connect(&database_url()?)
                .await
                .context("connexion à PostgreSQL impossible")?;
            kollega_store::run_migrations(&pool)
                .await
                .context("échec de l'application des migrations")?;
            println!("migrations appliquées");
        }
        Command::Version => println!("kollega {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        // clap vérifie la cohérence de la définition (conflits, doublons).
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
