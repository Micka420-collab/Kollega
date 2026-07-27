//! Les contraintes DÉCLARATIVES du schéma sont éprouvées, pas supposées.
//!
//! Trou trouvé le 29/07 en relisant les migrations : **aucun test
//! n'essayait jamais de violer une contrainte**. Les tests existants
//! prouvent que l'application ne produit pas d'état interdit ; aucun ne
//! prouvait que la BASE le refuserait si l'application s'égarait.
//!
//! La nuance décide de tout pour l'invariant 5. Le commentaire de la
//! migration 0003 affirme « le découvert est impossible » — c'est une
//! promesse du SCHÉMA, pas du code. Si une migration future retirait le
//! `CHECK`, la concurrence resterait verte (elle éprouve le verrou, pas la
//! contrainte) et la dernière ligne de défense tomberait en silence.
//!
//! Deux contraintes éprouvées ici, chacune parce qu'elle porte une
//! intention qu'on ne peut pas lire dans le code applicatif :
//!
//! 1. `credits.balance_cents CHECK (>= 0)` — un client ne consomme jamais
//!    à découvert, quoi que fasse la couche au-dessus.
//! 2. `users UNIQUE (org_id, email)` — l'unicité est PAR organisation, et
//!    délibérément pas globale : la migration 0002 explique qu'un email
//!    unique au niveau global divulguerait à un client l'existence d'un
//!    autre. Ce test vérifie les DEUX moitiés — le doublon refusé dans une
//!    organisation, le même email accepté dans une autre. Sans la seconde,
//!    on pourrait « durcir » en unicité globale sans rien casser, et créer
//!    la fuite que le schéma évitait.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use sqlx::{Connection, PgConnection};
use uuid::Uuid;

const APP_PASSWORD: &str = "kollega_app_test_pw";

fn app_url_from(migrate_url: &str) -> String {
    use sqlx::postgres::PgConnectOptions;
    use std::str::FromStr;
    let opts = PgConnectOptions::from_str(migrate_url).expect("URL invalide");
    format!(
        "postgres://kollega_app:{APP_PASSWORD}@{}:{}/{}",
        opts.get_host(),
        opts.get_port(),
        opts.get_database().unwrap_or("kollega"),
    )
}

/// Vrai si l'erreur vient bien de la CONTRAINTE nommée, et non d'un échec
/// quelconque : un test qui se contente de « ça a échoué » passerait aussi
/// sur une faute de frappe SQL.
fn is_constraint_violation(error: &sqlx::Error, code: &str) -> bool {
    error
        .as_database_error()
        .and_then(|e| e.code())
        .is_some_and(|c| c == code)
}

#[tokio::test]
async fn the_schema_itself_refuses_an_overdraft_and_a_duplicate_email() {
    let Ok(migrate_url) = std::env::var("TEST_MIGRATE_DATABASE_URL") else {
        eprintln!("IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — exécuté en CI.");
        return;
    };

    kollega_store::run_migrations(&migrate_url)
        .await
        .expect("migrations");
    kollega_store::set_app_role_password(&migrate_url, APP_PASSWORD)
        .await
        .expect("mot de passe kollega_app");

    let org_a = Uuid::from_u128(0x0005_C11A);
    let org_b = Uuid::from_u128(0x0005_C11B);
    let mut admin = PgConnection::connect(&migrate_url).await.expect("admin");
    for org in [org_a, org_b] {
        // Ordre imposé par les clés étrangères : les effets avant les
        // tâches, les tâches avant l'organisation. Sans cela, une seconde
        // exécution sur la même base échouerait au nettoyage, et le test
        // paraîtrait cassé alors qu'il aurait seulement déjà tourné.
        for table in [
            "tool_call_effects",
            "audit_chain",
            "audit_content",
            "tasks",
            "credits",
            "users",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE org_id = $1"))
                .bind(org)
                .execute(&mut admin)
                .await
                .expect("nettoyage");
        }
        sqlx::query("DELETE FROM organizations WHERE id = $1")
            .bind(org)
            .execute(&mut admin)
            .await
            .expect("nettoyage org");
    }

    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion");

    for (org, nom) in [(org_a, "Org Contraintes A"), (org_b, "Org Contraintes B")] {
        let mut tx = db.org_transaction(org).await.expect("tx");
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(org)
            .bind(nom)
            .execute(&mut *tx)
            .await
            .expect("organisation");
        tx.commit().await.expect("commit");
    }

    // 1. Le découvert, refusé par le SCHÉMA.
    let mut tx = db.org_transaction(org_a).await.expect("tx");
    let refus = sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, -1)")
        .bind(org_a)
        .execute(&mut *tx)
        .await
        .expect_err("un solde négatif doit être refusé À L'INSERTION");
    assert!(
        is_constraint_violation(&refus, "23514"),
        "l'insertion devait échouer sur la CONTRAINTE de vérification \
         (23514), pas pour une autre raison : {refus}"
    );
    drop(tx);

    // Et refusé aussi par mise à jour — c'est le chemin que prend le
    // pilote, donc celui qui compte en exploitation.
    let mut tx = db.org_transaction(org_a).await.expect("tx");
    sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 100)")
        .bind(org_a)
        .execute(&mut *tx)
        .await
        .expect("crédit initial");
    let refus = sqlx::query("UPDATE credits SET balance_cents = -1 WHERE org_id = $1")
        .bind(org_a)
        .execute(&mut *tx)
        .await
        .expect_err("un solde négatif doit être refusé À LA MISE À JOUR");
    assert!(
        is_constraint_violation(&refus, "23514"),
        "la mise à jour devait échouer sur la contrainte (23514) : {refus}"
    );
    drop(tx);

    // 2. L'email : doublon refusé DANS une organisation…
    let mut tx = db.org_transaction(org_a).await.expect("tx");
    sqlx::query(
        "INSERT INTO users (id, org_id, email, role) \
         VALUES ($1, $2, 'doublon@exemple.test', 'proprietaire')",
    )
    .bind(Uuid::new_v4())
    .bind(org_a)
    .execute(&mut *tx)
    .await
    .expect("premier utilisateur");
    let refus = sqlx::query(
        "INSERT INTO users (id, org_id, email, role) \
         VALUES ($1, $2, 'doublon@exemple.test', 'proprietaire')",
    )
    .bind(Uuid::new_v4())
    .bind(org_a)
    .execute(&mut *tx)
    .await
    .expect_err("deux fois le même email dans la MÊME organisation");
    assert!(
        is_constraint_violation(&refus, "23505"),
        "le doublon devait échouer sur l'unicité (23505) : {refus}"
    );
    drop(tx);

    // …et le MÊME email accepté dans une AUTRE organisation. C'est la
    // moitié qu'on oublie de tester, et celle qui porte l'intention : une
    // unicité globale dirait à un client qu'un autre existe.
    let mut tx = db.org_transaction(org_a).await.expect("tx");
    sqlx::query(
        "INSERT INTO users (id, org_id, email, role) \
         VALUES ($1, $2, 'doublon@exemple.test', 'proprietaire')",
    )
    .bind(Uuid::new_v4())
    .bind(org_a)
    .execute(&mut *tx)
    .await
    .expect("utilisateur de A");
    tx.commit().await.expect("commit A");

    let mut tx = db.org_transaction(org_b).await.expect("tx");
    sqlx::query(
        "INSERT INTO users (id, org_id, email, role) \
         VALUES ($1, $2, 'doublon@exemple.test', 'proprietaire')",
    )
    .bind(Uuid::new_v4())
    .bind(org_b)
    .execute(&mut *tx)
    .await
    .expect("le même email dans une AUTRE organisation doit être ACCEPTÉ");
    tx.commit().await.expect("commit B");

    // 3. Le « deuxième filet » de la migration 0004, éprouvé pour lui-même.
    //
    // Son commentaire annonce qu'il est INDÉPENDANT de la dérivation :
    // « même si le calcul de l'identité changeait, deux effets ne pourraient
    // pas coexister pour un même (tâche, itération) ». Rien ne le vérifiait.
    // Les tests d'idempotence existants passent par `derive_tool_call_id`, si
    // bien qu'ils ne produisent JAMAIS deux identités différentes pour la
    // même itération — ils ne peuvent donc pas atteindre cette contrainte.
    // Elle aurait pu disparaître d'une migration sans qu'aucun rouge n'en
    // parle, et l'idempotence n'aurait plus reposé que sur la dérivation.
    let task = Uuid::from_u128(0x0005_C11C);
    kollega_store::driver::create_task(&db, org_a, task, kollega_core::Cents(10_000), 4)
        .await
        .expect("tâche");

    let mut tx = db.org_transaction(org_a).await.expect("tx");
    let inserer = |tool_call_id: Uuid| {
        sqlx::query(
            "INSERT INTO tool_call_effects \
             (org_id, tool_call_id, task_id, iteration, tool, result_digest) \
             VALUES ($1, $2, $3, 7, 'mail.send', $4)",
        )
        .bind(org_a)
        .bind(tool_call_id)
        .bind(task)
        .bind(vec![0u8; 32])
    };
    inserer(Uuid::from_u128(0x0AAA))
        .execute(&mut *tx)
        .await
        .expect("premier effet");
    // Identité DIFFÉRENTE, même (tâche, itération) : c'est le cas que la
    // dérivation rend impossible aujourd'hui et que le schéma doit refuser
    // quand même.
    let refus = inserer(Uuid::from_u128(0x0BBB))
        .execute(&mut *tx)
        .await
        .expect_err("deux effets pour la même (tâche, itération)");
    assert!(
        is_constraint_violation(&refus, "23505"),
        "le second effet devait échouer sur l'unicité (23505), \
         indépendamment de la façon dont l'identité est calculée : {refus}"
    );
    drop(tx);

    // 4. Aucun effet ne peut pointer une tâche inexistante : sans cette
    // clé étrangère, une purge ou un export par organisation laisserait
    // derrière lui des lignes que plus rien ne rattache.
    let mut tx = db.org_transaction(org_a).await.expect("tx");
    let refus = sqlx::query(
        "INSERT INTO tool_call_effects \
         (org_id, tool_call_id, task_id, iteration, tool, result_digest) \
         VALUES ($1, $2, $3, 0, 'mail.send', $4)",
    )
    .bind(org_a)
    .bind(Uuid::from_u128(0x0CCC))
    .bind(Uuid::from_u128(0xDEAD_BEEF))
    .bind(vec![0u8; 32])
    .execute(&mut *tx)
    .await
    .expect_err("un effet sur une tâche inexistante");
    assert!(
        is_constraint_violation(&refus, "23503"),
        "l'insertion devait échouer sur la clé étrangère (23503) : {refus}"
    );
    drop(tx);
}
