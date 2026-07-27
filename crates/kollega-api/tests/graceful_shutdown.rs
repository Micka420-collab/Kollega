//! Le serveur s'arrête PROPREMENT, et sans base de données.
//!
//! Étape 1 de la tranche. Jusqu'ici `kollega serve` appelait
//! `axum::serve(...).await`, qui ne rend la main que sur erreur : un
//! `SIGTERM` — ce qu'envoient `docker stop`, systemd et l'ordonnanceur d'un
//! hébergeur avant de remplacer un conteneur — tuait le processus au milieu
//! des requêtes en vol.
//!
//! Ce que ça coûte pour CE produit : couper une requête entre l'attestation
//! d'intention et celle de résultat laisse dans la chaîne d'audit un appel
//! ouvert que plus rien ne réclamera. Le validateur de séquence le
//! signalerait comme une information, jamais comme une faute — donc
//! personne ne le verrait.
//!
//! **Aucune base n'est requise ici**, et c'est délibéré : ce test éprouve le
//! chemin d'écoute et d'arrêt, pas la persistance. Il fournit son propre
//! routeur, sans état, pour que l'absence de PostgreSQL ne l'empêche pas de
//! tourner — sur ce poste comme en intégration continue.

use std::time::Duration;

use axum::routing::get;
use axum::Router;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

#[tokio::test]
async fn the_server_answers_then_stops_cleanly_on_the_shutdown_signal() {
    // Routeur sans état : le propos est l'écoute et l'arrêt.
    let app = Router::new().route("/vivant", get(|| async { "vivant" }));

    // Port 0 : le système en attribue un libre — pas d'échec dû à un autre
    // processus occupant un numéro figé.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("écoute");
    let addr = listener.local_addr().expect("adresse locale");

    let (arret, attendre_arret) = tokio::sync::oneshot::channel::<()>();
    let serveur = tokio::spawn(async move {
        kollega_api::serve_until(listener, app, async {
            attendre_arret.await.ok();
        })
        .await
    });

    // 1. Il répond vraiment, sur une vraie socket.
    let mut socket = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connexion au serveur");
    socket
        .write_all(b"GET /vivant HTTP/1.1\r\nHost: kollega.test\r\nConnection: close\r\n\r\n")
        .await
        .expect("envoi de la requête");
    let mut reponse = Vec::new();
    socket
        .read_to_end(&mut reponse)
        .await
        .expect("lecture de la réponse");
    let reponse = String::from_utf8_lossy(&reponse);
    assert!(
        reponse.starts_with("HTTP/1.1 200 OK"),
        "le serveur devait répondre 200, reçu :\n{reponse}"
    );
    assert!(
        reponse.contains("\r\n\r\nvivant"),
        "le corps devait valoir « vivant » — un 200 vide ne prouverait pas \
         que la route a été atteinte :\n{reponse}"
    );

    // 2. LE POINT : l'arrêt demandé fait rendre la main, et vite.
    //
    // Sans `with_graceful_shutdown`, ce futur ne s'achèverait JAMAIS et le
    // test expirerait ici. C'est ce qui distingue « le serveur s'arrête »
    // de « le processus est tué ».
    arret.send(()).expect("demande d'arrêt");
    let issue = tokio::time::timeout(Duration::from_secs(5), serveur)
        .await
        .expect("le serveur doit rendre la main dans les cinq secondes")
        .expect("la tâche du serveur ne doit pas paniquer");
    assert!(
        issue.is_ok(),
        "un arrêt DEMANDÉ n'est pas une panne : {issue:?}"
    );

    // 3. Et il n'écoute plus : la socket est rendue.
    //
    // Sans cette vérification, un serveur qui rend la main tout en gardant
    // le port ouvert passerait pour arrêté — et le redéploiement suivant
    // échouerait à se lier, sans que rien ici ne l'ait annoncé.
    //
    // Ce qu'on exclut, c'est une connexion ACCEPTÉE. Un refus immédiat et
    // une expiration disent la même chose — personne n'écoute — et lequel
    // des deux survient dépend du système : sur ce poste Windows la
    // tentative expire là où Linux refuse aussitôt. Exiger le refus rendrait
    // le test vert ou rouge selon la plateforme, sans rien dire du serveur.
    let apres =
        tokio::time::timeout(Duration::from_secs(2), tokio::net::TcpStream::connect(addr)).await;
    assert!(
        !matches!(apres, Ok(Ok(_))),
        "après l'arrêt, plus rien ne doit ACCEPTER de connexion sur {addr}"
    );
}
