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

/// Le trait ne suffit pas si le pilote peut écrire du SQL à côté : la
/// chaîne d'audit ne doit subir AUCUN retrait ni AUCUNE réécriture, où que
/// ce soit dans la persistance. Les GRANT le refusent déjà côté base ; ce
/// test le refuse côté source, où l'intention se lit.
#[test]
fn no_sql_ever_removes_or_rewrites_a_chain_entry() {
    let store = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("kollega-store/src");
    let mut sources = Vec::new();
    collect_rust_sources(&store, &mut sources);
    assert!(!sources.is_empty(), "balayage suspect");

    let mut violations = Vec::new();
    for path in &sources {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()));
        let flat = content.to_ascii_uppercase().replace(['\n', '\r'], " ");
        for forbidden in [
            "DELETE FROM AUDIT_CHAIN",
            "UPDATE AUDIT_CHAIN",
            "TRUNCATE AUDIT_CHAIN",
            "DELETE FROM TOOL_CALL_EFFECTS",
            "UPDATE TOOL_CALL_EFFECTS",
        ] {
            if flat.contains(forbidden) {
                violations.push(format!("{} : {forbidden}", path.display()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "la persistance tente de retirer ou réécrire une preuve :\n{}",
        violations.join("\n")
    );
}

fn collect_rust_sources(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}
