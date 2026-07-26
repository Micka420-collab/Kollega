// NON VÉRIFIÉ — nécessite une base PostgreSQL, à exécuter en CI.
//
// Ce test structurel n'a jamais tourné sur la machine de développement
// (aucun PostgreSQL disponible). Sa première exécution réelle aura lieu en
// intégration continue, où TEST_MIGRATE_DATABASE_URL est fournie.

//! Filet structurel de l'invariant 1 : TOUTE table tenant a sa RLS.
//!
//! Balaie les catalogues (`pg_class`, `pg_attribute`, `pg_policy`) et échoue
//! si une table du schéma `public` portant une colonne `org_id` (ou la table
//! `organizations`, dont la clé primaire EST l'organisation) n'a pas :
//! `ENABLE ROW LEVEL SECURITY` + `FORCE ROW LEVEL SECURITY` + au moins une
//! politique. Contrairement au test d'isolation (qui prouve le comportement
//! sur organizations/users), ce filet couvre automatiquement chaque table
//! FUTURE : oublier la RLS dans une migration fera échouer la CI.

use sqlx::{Connection, PgConnection, Row};

/// Tables volontairement HORS RLS.
///
/// VIDE aujourd'hui, et ce doit être difficile à remplir : toute entrée
/// future exige une justification écrite ici même, en commentaire, avec la
/// référence de l'ADR qui l'autorise (ex. la future `login_identities`,
/// ADR-0005 — qui n'existe qu'en texte tant que sa migration n'est pas
/// écrite). Une entrée sans justification doit être refusée en revue.
const ALLOWED_WITHOUT_RLS: &[&str] = &[];

#[tokio::test]
async fn every_tenant_table_has_forced_rls_and_a_policy() {
    let Ok(migrate_url) = std::env::var("TEST_MIGRATE_DATABASE_URL") else {
        eprintln!(
            "IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — le filet structurel \
             RLS exige une base réelle (exécuté en CI)."
        );
        return;
    };

    kollega_store::run_migrations(&migrate_url)
        .await
        .expect("migrations");
    let mut conn = PgConnection::connect(&migrate_url)
        .await
        .expect("connexion");

    let rows = sqlx::query(
        "SELECT c.relname::text,
                c.relrowsecurity,
                c.relforcerowsecurity,
                (SELECT count(*) FROM pg_policy p WHERE p.polrelid = c.oid) AS policies
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public'
           AND c.relkind = 'r'
           AND (
             EXISTS (
               SELECT 1 FROM pg_attribute a
               WHERE a.attrelid = c.oid
                 AND a.attname = 'org_id'
                 AND NOT a.attisdropped
             )
             OR c.relname = 'organizations'
           )
         ORDER BY c.relname",
    )
    .fetch_all(&mut conn)
    .await
    .expect("catalogues PostgreSQL");

    assert!(
        !rows.is_empty(),
        "aucune table tenant trouvée : le filet ne surveille rien"
    );

    let mut failures = Vec::new();
    for row in &rows {
        let table: String = row.get(0);
        if ALLOWED_WITHOUT_RLS.contains(&table.as_str()) {
            continue;
        }
        let rls: bool = row.get(1);
        let force: bool = row.get(2);
        let policies: i64 = row.get(3);
        if !rls {
            failures.push(format!("{table} : RLS non activée"));
        }
        if !force {
            failures.push(format!("{table} : RLS non forcée (FORCE absent)"));
        }
        if policies == 0 {
            failures.push(format!("{table} : aucune politique"));
        }
    }
    assert!(
        failures.is_empty(),
        "tables tenant sans protection complète :\n{}",
        failures.join("\n")
    );
}
