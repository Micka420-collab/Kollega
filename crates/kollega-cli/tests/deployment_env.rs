//! Garde : ce que le binaire EXIGE de l'environnement, le déploiement le
//! fournit.
//!
//! `deploy/compose.yaml` est le seul chemin de démarrage documenté du
//! produit — et il n'est exécuté nulle part : la CI construit l'image, elle
//! ne lance pas la composition. Renommer une variable d'environnement dans
//! `main.rs` le casserait donc en silence, et la panne n'apparaîtrait qu'au
//! premier déploiement, c'est-à-dire au pire moment.
//!
//! Les quatre variables concordent aujourd'hui (vérifié à la main le
//! 29/07) ; ce test transforme cette vérification ponctuelle en propriété
//! tenue.
//!
//! Sens unique, à dessein : tout ce que le binaire LIT doit être fourni.
//! L'inverse n'est pas vrai et ne doit pas l'être — la composition définit
//! aussi `POSTGRES_USER` et consorts pour l'image PostgreSQL, qui ne nous
//! regardent pas.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("racine du dépôt")
        .to_path_buf()
}

/// Noms suivant un motif donné, entre guillemets : `env_var("X")`,
/// `env = "X"`.
fn names_after(source: &str, needle: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = source;
    while let Some(at) = rest.find(needle) {
        let after = &rest[at + needle.len()..];
        rest = after;
        let Some(open) = after.find('"').filter(|at| *at < 4) else {
            continue;
        };
        let Some(len) = after[open + 1..].find('"') else {
            break;
        };
        let name = &after[open + 1..open + 1 + len];
        if name.chars().all(|c| c.is_ascii_uppercase() || c == '_') && name.len() > 3 {
            out.insert(name.to_owned());
        }
    }
    out
}

#[test]
fn the_deployment_provides_every_variable_the_binary_reads() {
    let root = repo_root();
    let main_rs = fs::read_to_string(root.join("crates/kollega-cli/src/main.rs"))
        .expect("main.rs doit exister");

    let mut required = names_after(&main_rs, "env_var(");
    required.extend(names_after(&main_rs, "env = "));
    // `KOLLEGA_APP_DB_PASSWORD` est lu par une lecture directe de
    // l'environnement — la seule du binaire, parce que son absence n'est pas
    // une erreur mais un mode de fonctionnement (« pose le mot de passe si
    // tu l'as »).
    //
    // Motif assemblé à la compilation : écrit en clair, il ferait rougir la
    // garde `integration_tests_ran`, qui cherche justement ce texte dans les
    // fichiers de test pour y traquer les sauts conditionnels. Elle s'était
    // tendu le même piège à elle-même ; la parade est la même.
    required.extend(names_after(&main_rs, concat!("std::env", "::var(")));
    assert!(
        required.len() >= 4,
        "extraction suspecte : {required:?} — le binaire lit au moins \
         DATABASE_URL, KOLLEGA_MIGRATE_DATABASE_URL, KOLLEGA_APP_DB_PASSWORD \
         et KOLLEGA_LISTEN"
    );

    let compose = fs::read_to_string(root.join("deploy/compose.yaml"))
        .expect("deploy/compose.yaml doit exister");
    let provided: BTreeSet<String> = compose
        .lines()
        .filter_map(|line| line.trim().split_once(':'))
        .map(|(key, _)| key.trim().to_owned())
        .filter(|key| !key.is_empty() && key.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .collect();

    let missing: Vec<&String> = required.difference(&provided).collect();
    assert!(
        missing.is_empty(),
        "le binaire lit des variables que le déploiement ne fournit pas : \
         {missing:?}. `deploy/compose.yaml` est le seul chemin de démarrage \
         documenté et il n'est exécuté nulle part — la panne n'apparaîtrait \
         qu'au premier déploiement. Fournies : {provided:?}"
    );
}
