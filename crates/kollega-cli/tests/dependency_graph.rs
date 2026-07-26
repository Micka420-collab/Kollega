//! Vérification du graphe de dépendances imposé (CI).
//!
//! Ce test ÉCHOUE si une dépendance interdite apparaît :
//! - une arête interne hors du graphe `core → policy/audit/memory/tools/model
//!   → runtime → store, api → cli` ;
//! - une dépendance externe de `kollega-core` hors de la liste blanche
//!   (invariant 9 : notamment sqlx, reqwest ou tokio dans le domaine).

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// Seules dépendances externes autorisées dans kollega-core (invariant 9).
const CORE_EXTERNAL_ALLOWED: &[&str] = &["serde", "thiserror", "uuid", "time"];

/// Arêtes internes autorisées : crate -> dépendances kollega-* permises.
fn allowed_internal(name: &str) -> &'static [&'static str] {
    const LOWER: &[&str] = &["kollega-core"];
    const MIDDLE: &[&str] = &[
        "kollega-core",
        "kollega-policy",
        "kollega-audit",
        "kollega-memory",
        "kollega-tools",
        "kollega-model",
    ];
    const UPPER: &[&str] = &[
        "kollega-core",
        "kollega-policy",
        "kollega-audit",
        "kollega-memory",
        "kollega-tools",
        "kollega-model",
        "kollega-runtime",
    ];
    // L'API accède aux données via le point de passage unique de kollega-store
    // (invariant 1) : l'arête api -> store est la seule permise entre ces deux
    // crates, jamais l'inverse.
    const UPPER_AND_STORE: &[&str] = &[
        "kollega-core",
        "kollega-policy",
        "kollega-audit",
        "kollega-memory",
        "kollega-tools",
        "kollega-model",
        "kollega-runtime",
        "kollega-store",
    ];
    const ALL: &[&str] = &[
        "kollega-core",
        "kollega-policy",
        "kollega-audit",
        "kollega-memory",
        "kollega-tools",
        "kollega-model",
        "kollega-runtime",
        "kollega-store",
        "kollega-api",
    ];
    match name {
        "kollega-core" => &[],
        "kollega-policy" | "kollega-audit" | "kollega-memory" | "kollega-tools"
        | "kollega-model" => LOWER,
        "kollega-runtime" => MIDDLE,
        "kollega-store" => UPPER,
        "kollega-api" => UPPER_AND_STORE,
        "kollega-cli" => ALL,
        other => panic!("crate inconnue dans le workspace : {other}"),
    }
}

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ doit exister")
        .to_path_buf()
}

fn dependency_names(manifest: &toml::Value, section: &str) -> BTreeSet<String> {
    manifest
        .get(section)
        .and_then(|d| d.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn dependency_graph_is_respected() {
    let mut seen = 0;
    for entry in fs::read_dir(crates_dir()).expect("lecture de crates/") {
        let path = entry.expect("entrée lisible").path().join("Cargo.toml");
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()));
        let manifest: toml::Value = raw
            .parse()
            .unwrap_or_else(|e| panic!("TOML invalide dans {} : {e}", path.display()));
        let name = manifest["package"]["name"]
            .as_str()
            .expect("package.name présent")
            .to_owned();
        seen += 1;

        let allowed = allowed_internal(&name);
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            for dep in dependency_names(&manifest, section) {
                if dep.starts_with("kollega-") {
                    assert!(
                        allowed.contains(&dep.as_str()),
                        "arête interdite dans le graphe : {name} -> {dep} ({section})"
                    );
                }
            }
        }

        if name == "kollega-core" {
            for dep in dependency_names(&manifest, "dependencies") {
                assert!(
                    CORE_EXTERNAL_ALLOWED.contains(&dep.as_str()),
                    "invariant 9 violé : kollega-core dépend de {dep} \
                     (autorisées : {CORE_EXTERNAL_ALLOWED:?})"
                );
            }
        }
    }
    assert_eq!(seen, 10, "le workspace doit contenir exactement 10 crates");
}
