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
    /// Vérifie le journal d'audit d'une organisation (invariant 4).
    ///
    /// Deux questions distinctes, posées l'une après l'autre :
    /// 1. la chaîne a-t-elle été ALTÉRÉE ? (hachages, liens, hauteurs)
    /// 2. ce qu'elle raconte a-t-il du SENS ? (séquence des appels d'outils)
    ///
    /// Sort en code 1 dès qu'une des deux échoue : la commande est faite
    /// pour être branchée à une supervision, pas seulement lue.
    Audit {
        #[command(subcommand)]
        command: AuditCommand,
    },
    /// Affiche la version du binaire.
    Version,
}

#[derive(Subcommand)]
enum AuditCommand {
    /// Vérifie la chaîne et la séquence d'une organisation.
    Verify {
        /// Organisation à vérifier (UUID).
        #[arg(long)]
        org: uuid::Uuid,
    },
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
        Command::Audit {
            command: AuditCommand::Verify { org },
        } => {
            let db = kollega_store::Db::connect(&env_var("DATABASE_URL")?)
                .await
                .context("connexion à PostgreSQL impossible")?;

            // 1. Intégrité de la chaîne.
            match kollega_store::driver::verify_org_chain(&db, org).await {
                Ok(check) => println!("chaîne : {} entrées, intègre", check.entries),
                Err(e) => {
                    // Une rupture n'est pas un plantage : on la NOMME, et on
                    // sort en échec pour qu'une supervision la voie.
                    eprintln!("chaîne : ROMPUE — {e}");
                    std::process::exit(1);
                }
            }

            // 2. Cohérence de ce que la chaîne raconte.
            let report = kollega_store::driver::verify_org_sequence(&db, org)
                .await
                .context("lecture de la séquence d'appels impossible")?;
            if !report.open_calls.is_empty() {
                // INFORMATION, pas faute : une validation en attente ou un
                // redémarrage laisse légitimement des appels ouverts.
                println!("séquence : {} appel(s) ouvert(s)", report.open_calls.len());
                for call in &report.open_calls {
                    println!("  ouvert : {call}");
                }
            }
            if report.violations.is_empty() {
                println!("séquence : cohérente");
            } else {
                eprintln!("séquence : {} VIOLATION(S)", report.violations.len());
                for violation in &report.violations {
                    eprintln!(
                        "  position {} — {:?} (appel {})",
                        violation.position, violation.kind, violation.tool_call_id
                    );
                }
                std::process::exit(1);
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
