//! Le serveur HTTP DÉMARRE réellement, et répond.
//!
//! Trou trouvé le 29/07 : `kollega serve` est la commande par défaut de
//! l'image publiée et signée — et **aucun test n'avait jamais démarré ce
//! serveur**. `kollega-api` ne contenait qu'un `fn crate_compiles() {}`,
//! c'est-à-dire un test qui ne prouve rien de plus que le compilateur.
//! Toute la chaîne de livraison (construction, SBOM, signature,
//! vérification) portait donc sur un binaire dont personne n'avait vérifié
//! que sa commande par défaut savait se lever.
//!
//! Ce test prend le chemin RÉEL, pas un raccourci : il ouvre une vraie
//! socket, lance `axum::serve`, et parle HTTP en clair dessus — exactement
//! ce que fait `main.rs`. Passer par un appel direct du gestionnaire aurait
//! laissé hors de portée précisément ce qui n'a jamais tourné : l'écoute
//! et le service.
//!
//! **Ce qu'il ne couvre pas, et pourquoi** : la branche dégradée (503,
//! « base de données injoignable »). `Db::connect` établit la connexion
//! immédiatement, donc un pool connecté-mais-mort n'est pas représentable
//! sans ajouter au produit une surface qu'aucun besoin ne justifie. La
//! branche reste donc non couverte, et c'est écrit plutôt que masqué.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon — et
//! `integration_tests_ran.rs` fait de ce saut un échec en CI.

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

const APP_PASSWORD: &str = "kollega_app_test_pw";

fn app_url_from(migrate_url: &str) -> String {
    use sqlx::postgres::PgConnectOptions;
    use std::str::FromStr;
    let opts = PgConnectOptions::from_str(migrate_url).expect("TEST_MIGRATE_DATABASE_URL invalide");
    format!(
        "postgres://kollega_app:{APP_PASSWORD}@{}:{}/{}",
        opts.get_host(),
        opts.get_port(),
        opts.get_database().unwrap_or("kollega")
    )
}

#[tokio::test]
async fn the_server_actually_starts_and_answers_on_health() {
    let Ok(migrate_url) = std::env::var("TEST_MIGRATE_DATABASE_URL") else {
        eprintln!("IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — exécuté en CI.");
        return;
    };

    kollega_store::run_migrations(&migrate_url)
        .await
        .expect("migrations");
    kollega_store::set_app_role_password(&migrate_url, APP_PASSWORD)
        .await
        .expect("mot de passe du rôle applicatif");

    // Le rôle de PRODUCTION, kollega_app — pas celui des migrations : le
    // but est d'éprouver le chemin que le binaire prend réellement.
    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion applicative");

    // Port 0 : le système en attribue un libre, donc pas de test qui
    // échoue parce qu'un autre processus occupait un numéro figé.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("écoute");
    let addr = listener.local_addr().expect("adresse locale");
    let server = tokio::spawn(async move {
        axum::serve(listener, kollega_api::router(db, None))
            .await
            .ok();
    });

    let mut socket = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connexion au serveur");
    socket
        .write_all(b"GET /health HTTP/1.1\r\nHost: kollega.test\r\nConnection: close\r\n\r\n")
        .await
        .expect("envoi de la requête");
    let mut response = Vec::new();
    socket
        .read_to_end(&mut response)
        .await
        .expect("lecture de la réponse");
    let response = String::from_utf8_lossy(&response);

    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "le serveur devait répondre 200 sur /health, réponse reçue :\n{response}"
    );
    // Le corps, pas seulement le code : un 200 avec un corps vide voudrait
    // dire que la route répond sans que la base ait rien confirmé.
    assert!(
        response.contains("\r\n\r\nok"),
        "le corps devait valoir « ok » (la base a répondu au SELECT 1) :\n{response}"
    );

    server.abort();
}
