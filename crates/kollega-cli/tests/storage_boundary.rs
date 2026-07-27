//! Garde de la frontière de stockage (bloc 3a).
//!
//! `ContentDigest::from_storage` relit une empreinte SANS la recalculer :
//! c'est un acte de confiance envers le stockage, légitime UNIQUEMENT dans
//! le crate de persistance (qui active la feature `storage-boundary`).
//! La feature seule ne suffit pas — l'unification des features de cargo la
//! rend visible à tout le graphe une fois activée quelque part — donc ce
//! test textuel échoue si `from_storage` est INVOQUÉ hors de
//! `kollega-audit` (sa définition) et `kollega-store` (son usage légitime).

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_crates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ doit exister")
        .to_path_buf()
}

fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "target" && name != ".git" {
                collect_rust_sources(&path, out);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn from_storage_is_only_reachable_from_the_persistence_crate() {
    let crates = workspace_crates();
    let mut sources = Vec::new();
    collect_rust_sources(&crates, &mut sources);
    assert!(sources.len() >= 20, "balayage suspect");

    let needle = "from_storage";
    let mut violations = Vec::new();
    for path in &sources {
        let rel = path.strip_prefix(&crates).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        // Autorisés : la définition (kollega-audit) et la persistance
        // (kollega-store). Ce fichier-ci parle du nom sous forme de donnée.
        if rel_str.starts_with("kollega-audit/")
            || rel_str.starts_with("kollega-store/")
            || rel_str.ends_with("storage_boundary.rs")
        {
            continue;
        }
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()));
        if content.contains(needle) {
            violations.push(rel_str);
        }
    }
    assert!(
        violations.is_empty(),
        "la frontière de stockage a fui hors de la persistance : {violations:?}"
    );
}
