//! Étape 2 : la base est branchée AU SERVEUR, pas seulement au pilote.
//!
//! Jusqu'ici le serveur n'avait qu'une route `/health`, dont le `SELECT 1`
//! ne touche aucune table. Le pool était donc connecté sans qu'aucun chemin
//! HTTP n'atteigne une donnée de tenant : « la base est branchée » était
//! vrai du pilote, pas du service.
//!
//! Ce test fait passer une ÉCRITURE et une LECTURE réelles par le serveur,
//! jusqu'à PostgreSQL, à travers `Db::org_transaction` — le point de
//! passage unique qui pose le contexte d'organisation.
//!
//! Il vérifie aussi ce qui compte le plus : demander la tâche d'une AUTRE
//! organisation rend `404`, exactement comme une tâche inexistante. C'est
//! l'invariant 1 éprouvé sur le chemin HTTP, et non plus seulement sur le
//! pilote.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use std::time::Duration;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
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

/// Envoie une requête brute et rend la réponse entière.
async fn requete(addr: std::net::SocketAddr, brut: &str) -> String {
    let mut socket = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connexion au serveur");
    socket
        .write_all(brut.as_bytes())
        .await
        .expect("envoi de la requête");
    let mut reponse = Vec::new();
    socket
        .read_to_end(&mut reponse)
        .await
        .expect("lecture de la réponse");
    String::from_utf8_lossy(&reponse).into_owned()
}

#[tokio::test]
async fn a_task_is_written_and_read_back_through_http_and_never_across_orgs() {
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

    let org = Uuid::from_u128(0x00_47_7B_01);
    let autre_org = Uuid::from_u128(0x00_47_7B_02);
    let task = Uuid::from_u128(0x00_47_7B_03);

    use sqlx::Connection as _;
    let mut admin = sqlx::PgConnection::connect(&migrate_url)
        .await
        .expect("admin");
    for o in [org, autre_org] {
        for table in [
            "tool_call_effects",
            "audit_chain",
            "audit_content",
            "tasks",
            "credits",
            "users",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE org_id = $1"))
                .bind(o)
                .execute(&mut admin)
                .await
                .expect("nettoyage");
        }
        sqlx::query("DELETE FROM organizations WHERE id = $1")
            .bind(o)
            .execute(&mut admin)
            .await
            .expect("nettoyage org");
    }

    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion applicative");
    for (o, nom) in [(org, "Org HTTP"), (autre_org, "Org Voisine")] {
        let mut tx = db.org_transaction(o).await.expect("tx");
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(o)
            .bind(nom)
            .execute(&mut *tx)
            .await
            .expect("organisation");
        sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 100000)")
            .bind(o)
            .execute(&mut *tx)
            .await
            .expect("crédit");
        tx.commit().await.expect("commit");
    }

    // Le VRAI routeur du produit, pas un montage de test.
    let app = kollega_api::router(db);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("écoute");
    let addr = listener.local_addr().expect("adresse locale");
    let (arret, attendre) = tokio::sync::oneshot::channel::<()>();
    let serveur = tokio::spawn(async move {
        kollega_api::serve_until(listener, app, async {
            attendre.await.ok();
        })
        .await
    });

    // 1. ÉCRITURE par HTTP.
    let corps = r#"{"ceiling_cents":10000,"max_iterations":4}"#;
    let reponse = requete(
        addr,
        &format!(
            "POST /orgs/{org}/tasks/{task} HTTP/1.1\r\nHost: k\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{corps}",
            corps.len()
        ),
    )
    .await;
    assert!(
        reponse.starts_with("HTTP/1.1 201 CREATED"),
        "la création devait répondre 201 :\n{reponse}"
    );

    // 2. LECTURE par HTTP, dans la bonne organisation.
    let reponse = requete(
        addr,
        &format!("GET /orgs/{org}/tasks/{task} HTTP/1.1\r\nHost: k\r\nConnection: close\r\n\r\n"),
    )
    .await;
    assert!(
        reponse.starts_with("HTTP/1.1 200 OK"),
        "la lecture devait répondre 200 :\n{reponse}"
    );
    assert!(
        reponse.contains("Pending"),
        "la tâche créée est en attente — le corps doit le dire :\n{reponse}"
    );

    // 3. LE POINT : la même tâche, demandée depuis une AUTRE organisation.
    //
    // Elle existe, mais la RLS la rend invisible depuis ce contexte. La
    // réponse doit être `404`, identique à celle d'une tâche inexistante :
    // toute réponse distincte apprendrait au voisin que cette tâche
    // existe, et permettrait de les énumérer sans jamais toucher à la RLS.
    let reponse = requete(
        addr,
        &format!(
            "GET /orgs/{autre_org}/tasks/{task} HTTP/1.1\r\nHost: k\r\nConnection: close\r\n\r\n"
        ),
    )
    .await;
    assert!(
        reponse.starts_with("HTTP/1.1 404 NOT FOUND"),
        "la tâche d'une AUTRE organisation doit être indiscernable d'une \
         tâche inexistante :\n{reponse}"
    );

    // 4. Et une tâche qui n'existe nulle part : même réponse.
    let inconnue = Uuid::from_u128(0x00_47_7B_FF);
    let reponse = requete(
        addr,
        &format!(
            "GET /orgs/{org}/tasks/{inconnue} HTTP/1.1\r\nHost: k\r\nConnection: close\r\n\r\n"
        ),
    )
    .await;
    assert!(
        reponse.starts_with("HTTP/1.1 404 NOT FOUND"),
        "une tâche inexistante doit rendre 404 :\n{reponse}"
    );

    arret.send(()).expect("demande d'arrêt");
    tokio::time::timeout(Duration::from_secs(5), serveur)
        .await
        .expect("le serveur doit rendre la main")
        .expect("la tâche du serveur ne doit pas paniquer")
        .expect("arrêt demandé, pas une panne");
}
