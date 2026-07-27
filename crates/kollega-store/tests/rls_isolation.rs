//! Test d'isolation multi-tenant (invariant 1) — la seule preuve qui compte.
//!
//! Nécessite une base PostgreSQL réelle avec pgvector, désignée par
//! `TEST_MIGRATE_DATABASE_URL` : un rôle qui applique les migrations et peut
//! exécuter le DDL du scénario de sensibilité — en CI, le superutilisateur
//! bootstrap du conteneur (en production, kollega_migrate est un propriétaire
//! NON superutilisateur, cf. migrations/0001). Sans cette variable, le test
//! est sauté avec un message explicite : il s'exécute en intégration continue.
//!
//! Déroulé :
//! 1. migrations appliquées, mot de passe du rôle `kollega_app` posé ;
//! 2. témoins structurels : RLS activée ET forcée sur chaque table tenant,
//!    kollega_app sans BYPASSRLS — détecte la perte d'ENABLE ou de FORCE et
//!    l'octroi de BYPASSRLS, quel que soit le propriétaire des tables ;
//! 3. données insérées pour deux organisations A et B via le point de passage
//!    unique (`Db::org_transaction`), chacune témoin de sa propre ligne ;
//! 4. dans le contexte de A, un SELECT sans clause WHERE ne voit QUE A ;
//! 5. hors de tout contexte, la requête ÉCHOUE (fermé par défaut) ;
//! 6. sensibilité : RLS désactivée à la main => la fuite devient visible —
//!    preuve que ce test échouerait si la RLS tombait — puis RLS réactivée.
//!
//! Une politique supprimée est aussi détectée : fermé par défaut, l'étape 4
//! ne verrait plus même les lignes de A.

use sqlx::{Connection, PgConnection, Row};
use uuid::Uuid;

const APP_PASSWORD: &str = "kollega_app_test_pw";

fn migrate_url() -> Option<String> {
    std::env::var("TEST_MIGRATE_DATABASE_URL").ok()
}

fn app_url_from(migrate_url: &str) -> String {
    use sqlx::postgres::PgConnectOptions;
    use std::str::FromStr;
    let opts = PgConnectOptions::from_str(migrate_url).expect("TEST_MIGRATE_DATABASE_URL invalide");
    format!(
        "postgres://kollega_app:{APP_PASSWORD}@{}:{}/{}",
        opts.get_host(),
        opts.get_port(),
        opts.get_database().unwrap_or("kollega"),
    )
}

async fn count(conn: &mut PgConnection, sql: &str) -> i64 {
    sqlx::query(sql)
        .fetch_one(conn)
        .await
        .expect("requête de comptage")
        .get::<i64, _>(0)
}

#[tokio::test]
async fn tenant_isolation_holds_and_the_test_is_sensitive() {
    let Some(migrate_url) = migrate_url() else {
        eprintln!(
            "IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — le test d'isolation \
             exige une base réelle (exécuté en CI)."
        );
        return;
    };

    // 1. Migrations, mot de passe applicatif, base propre.
    kollega_store::run_migrations(&migrate_url)
        .await
        .expect("migrations");
    kollega_store::set_app_role_password(&migrate_url, APP_PASSWORD)
        .await
        .expect("mot de passe kollega_app");

    let mut admin = PgConnection::connect(&migrate_url).await.expect("admin");
    // TRUNCATE n'est pas soumis aux politiques RLS : le rôle de migration
    // (propriétaire des tables, superutilisateur en CI) peut nettoyer.
    // CASCADE : depuis la migration 0003, tasks/audit_chain/audit_content/
    // credits référencent organizations — un TRUNCATE nu échouerait sur les
    // clés étrangères (trouvaille d'intégration de la tranche verticale).
    sqlx::query("TRUNCATE users, organizations CASCADE")
        .execute(&mut admin)
        .await
        .expect("nettoyage");

    // Témoins structurels : la RLS est activée ET forcée sur chaque table
    // tenant, et kollega_app n'a pas BYPASSRLS. Détecte la perte d'ENABLE,
    // la perte de FORCE (invisible autrement : le propriétaire de CI est
    // superutilisateur) et l'octroi de BYPASSRLS.
    let flags = sqlx::query(
        "SELECT relname::text, relrowsecurity, relforcerowsecurity
         FROM pg_class
         WHERE relname IN ('organizations', 'users') AND relkind = 'r'",
    )
    .fetch_all(&mut admin)
    .await
    .expect("catalogue pg_class");
    assert_eq!(flags.len(), 2, "les deux tables tenant doivent exister");
    for row in &flags {
        let table: String = row.get(0);
        assert!(row.get::<bool, _>(1), "RLS non activée sur {table}");
        assert!(row.get::<bool, _>(2), "RLS non forcée sur {table}");
    }
    let bypass: bool =
        sqlx::query("SELECT rolbypassrls FROM pg_roles WHERE rolname = 'kollega_app'")
            .fetch_one(&mut admin)
            .await
            .expect("pg_roles")
            .get(0);
    assert!(!bypass, "kollega_app ne doit jamais avoir BYPASSRLS");

    // 2. Deux organisations, insérées par le point de passage unique.
    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion kollega_app");
    let org_a = Uuid::from_u128(0xA);
    let org_b = Uuid::from_u128(0xB);
    for (org, name, email) in [
        (org_a, "Org A", "a@a.example"),
        (org_b, "Org B", "b@b.example"),
    ] {
        let mut tx = db.org_transaction(org).await.expect("transaction org");
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(org)
            .bind(name)
            .execute(&mut *tx)
            .await
            .expect("insertion organisation");
        sqlx::query(
            "INSERT INTO users (id, org_id, email, role) VALUES ($1, $2, $3, 'proprietaire')",
        )
        .bind(Uuid::new_v4())
        .bind(org)
        .bind(email)
        .execute(&mut *tx)
        .await
        .expect("insertion utilisateur");
        tx.commit().await.expect("commit");
    }

    // Témoin de données : chaque organisation voit sa propre ligne via le
    // point de passage unique — « zéro fuite » n'est pas « zéro donnée ».
    // Aucune hypothèse superutilisateur ici.
    for org in [org_a, org_b] {
        let mut tx = db.org_transaction(org).await.expect("transaction témoin");
        assert_eq!(
            count(&mut tx, "SELECT count(*) FROM organizations").await,
            1
        );
    }

    // 3. Dans le contexte de A : uniquement les lignes de A, sans clause WHERE.
    let mut tx = db.org_transaction(org_a).await.expect("transaction A");
    let orgs: Vec<Uuid> = sqlx::query("SELECT id FROM organizations")
        .fetch_all(&mut *tx)
        .await
        .expect("SELECT organizations")
        .into_iter()
        .map(|r| r.get::<Uuid, _>(0))
        .collect();
    assert_eq!(orgs, vec![org_a], "le contexte de A ne doit voir que A");
    let users_orgs: Vec<Uuid> = sqlx::query("SELECT org_id FROM users")
        .fetch_all(&mut *tx)
        .await
        .expect("SELECT users")
        .into_iter()
        .map(|r| r.get::<Uuid, _>(0))
        .collect();
    assert_eq!(users_orgs, vec![org_a], "aucun utilisateur d'une autre org");
    drop(tx);

    // 4. Hors contexte : la requête échoue — fermé par défaut, pas silencieux.
    let mut plain = PgConnection::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion sans contexte");
    let out_of_context = sqlx::query("SELECT count(*) FROM users")
        .fetch_one(&mut plain)
        .await;
    // On vérifie POURQUOI elle échoue, pas seulement qu'elle échoue : un
    // simple `is_err()` serait vert sur une connexion coupée, une faute de
    // frappe SQL ou un défaut de privilège — c'est-à-dire sans rien dire de
    // la RLS. La cause attendue est nommée : le réglage d'organisation
    // n'existe pas dans cette session, et `current_setting` sans `missing_ok`
    // le refuse (ADR-0002, point 3 : fermé par défaut, jamais silencieux).
    let erreur = out_of_context
        .expect_err("sans app.current_org, la requête doit échouer, pas retourner des lignes");
    let base = erreur
        .as_database_error()
        .expect("l'échec doit venir de la BASE, pas du transport");
    assert!(
        base.message().contains("app.current_org"),
        "l'échec doit nommer le réglage manquant — sinon il pourrait venir \
         de tout autre chose et ce test ne prouverait rien : {base}"
    );

    // 5. Sensibilité : si la RLS tombe, la fuite doit devenir visible —
    //    c'est la preuve que ce test échouerait en cas de régression.
    sqlx::query("ALTER TABLE users DISABLE ROW LEVEL SECURITY")
        .execute(&mut admin)
        .await
        .expect("désactivation RLS");
    let mut tx = db.org_transaction(org_a).await.expect("transaction A");
    let leaked = count(&mut tx, "SELECT count(*) FROM users").await;
    drop(tx);
    assert_eq!(
        leaked, 2,
        "RLS désactivée, la fuite doit apparaître — sinon ce test ne prouve rien"
    );

    // Restauration, puis re-vérification de l'isolation.
    sqlx::query("ALTER TABLE users ENABLE ROW LEVEL SECURITY")
        .execute(&mut admin)
        .await
        .expect("réactivation RLS");
    sqlx::query("ALTER TABLE users FORCE ROW LEVEL SECURITY")
        .execute(&mut admin)
        .await
        .expect("FORCE RLS");
    let mut tx = db.org_transaction(org_a).await.expect("transaction A");
    assert_eq!(count(&mut tx, "SELECT count(*) FROM users").await, 1);
}
