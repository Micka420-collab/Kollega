//! Garde de forme des dépôts d'audit (bloc 3f).
//!
//! Le dépôt de CHAÎNE ne doit jamais gagner une méthode de retrait : ce
//! test lit la définition du trait `AuditChainRepository` et échoue si un
//! identifiant de suppression y apparaît. La purge n'existe que sur le
//! dépôt de CONTENU, sous son nom propre (`purge_org`).

use std::fs;
use std::path::PathBuf;

#[test]
fn the_chain_repository_cannot_express_removal() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("kollega-audit/src/repository.rs");
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()));

    let chain_trait = source
        .split("pub trait AuditChainRepository")
        .nth(1)
        .and_then(|after| after.split("pub trait").next())
        .expect("le trait AuditChainRepository doit exister");

    for forbidden in [
        "delete", "remove", "purge", "truncate", "drop", "update", "rewrite", "clear",
    ] {
        assert!(
            !chain_trait.to_ascii_lowercase().contains(forbidden),
            "le dépôt de chaîne exprime un retrait : « {forbidden} » — l'ajout seul est \
             sa surface, pas sa discipline"
        );
    }
    // Les deux seules capacités admises existent bel et bien.
    assert!(chain_trait.contains("async fn append"));
    assert!(chain_trait.contains("async fn read"));
}
