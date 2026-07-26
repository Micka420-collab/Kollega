//! Binaire `kollega` : serve, migrate, version.

#![forbid(unsafe_code)]

use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kollega", version, about = "Plateforme d'agents IA gouvernés")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Démarre le serveur HTTP (rôle base : kollega_app, jamais un autre).
    Serve {
        /// Adresse d'écoute.
        #[arg(long, env = "KOLLEGA_LISTEN", default_value = "127.0.0.1:8080")]
        listen: String,
    },
    /// Applique les migrations (rôle base : kollega_migrate).
    ///
    /// Si KOLLEGA_APP_DB_PASSWORD est défini, pose ensuite le mot de passe du
    /// rôle applicatif kollega_app (le secret vient de l'environnement, jamais
    /// d'une migration).
    Migrate,
    /// Affiche la version du binaire.
    Version,
}

fn env_var(name: &str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| format!("la variable d'environnement {name} est absente"))
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
            let db = kollega_store::Db::connect(&env_var("DATABASE_URL")?)
                .await
                .context("connexion à PostgreSQL impossible")?;
            let app = kollega_api::router(db);
            let listener = tokio::net::TcpListener::bind(&listen)
                .await
                .with_context(|| format!("impossible d'écouter sur {listen}"))?;
            tracing::info!(%listen, "kollega démarre");
            axum::serve(listener, app)
                .await
                .context("le serveur HTTP s'est arrêté en erreur")?;
        }
        Command::Migrate => {
            let migrate_url = env_var("KOLLEGA_MIGRATE_DATABASE_URL")?;
            kollega_store::run_migrations(&migrate_url)
                .await
                .context("échec de l'application des migrations")?;
            println!("migrations appliquées");
            if let Ok(password) = std::env::var("KOLLEGA_APP_DB_PASSWORD") {
                kollega_store::set_app_role_password(&migrate_url, &password)
                    .await
                    .context("échec de la mise à jour du mot de passe de kollega_app")?;
                println!("mot de passe du rôle kollega_app synchronisé");
            }
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
