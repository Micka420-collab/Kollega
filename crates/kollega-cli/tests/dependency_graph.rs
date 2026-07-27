//! Vérification du graphe de dépendances imposé (CI).
//!
//! Ce test ÉCHOUE si une dépendance interdite apparaît :
//! - une arête interne hors du graphe `core → policy/audit/memory/tools/model
//!   → runtime → store, api → cli` ;
//! - une dépendance externe de `kollega-core` hors de la liste blanche, dans
//!   N'IMPORTE QUELLE section — dependencies, dev-dependencies,
//!   build-dependencies, y compris sous `[target.*]` (invariant 11 :
//!   notamment sqlx, reqwest ou tokio dans le domaine).

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// Seules dépendances externes autorisées dans kollega-core (invariant 11).
const CORE_EXTERNAL_ALLOWED: &[&str] = &["serde", "thiserror", "uuid", "time"];

/// En test uniquement, le domaine peut en plus utiliser un sérialiseur concret
/// (`serde_json`) et un framework de test de propriétés (`proptest`) — tous
/// deux purs, sans entrée-sortie, absents de l'artefact livré.
const CORE_DEV_ALLOWED: &[&str] = &[
    "serde",
    "thiserror",
    "uuid",
    "time",
    "serde_json",
    "proptest",
];

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

/// Noms de dépendances d'une section, EN INCLUANT les sections
/// `[target.'cfg(...)'.<section>]` : une dépendance conditionnée par cible
/// est une dépendance (et `cfg(all())` est vrai partout) — l'ignorer serait
/// un contournement silencieux du garde-fou.
fn dependency_names(manifest: &toml::Value, section: &str) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = manifest
        .get(section)
        .and_then(|d| d.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();
    if let Some(targets) = manifest.get("target").and_then(|t| t.as_table()) {
        for target_cfg in targets.values() {
            if let Some(table) = target_cfg.get(section).and_then(|d| d.as_table()) {
                out.extend(table.keys().cloned());
            }
        }
    }
    out
}

/// Crates d'ENTRÉE-SORTIE : réseau, fichiers, base, exécuteur async.
///
/// Leur présence dans la fermeture TRANSITIVE de `kollega-core` violerait
/// l'invariant 11, même si le manifeste du domaine reste impeccable.
///
/// `getrandom` n'y figure PAS, et c'est un jugement assumé : il fournit de
/// l'entropie (un appel système), pas de l'entrée-sortie au sens du
/// produit — et il n'est présent que parce que la PÉRIPHÉRIE utilise
/// `uuid` v4, l'unification des features de cargo le propageant à tout le
/// graphe. Le domaine, lui, ne déclare pas v4 et n'appelle jamais
/// `new_v4` (garde textuelle ci-dessous).
const IO_CRATES: &[&str] = &[
    "tokio",
    "sqlx",
    "sqlx-core",
    "sqlx-postgres",
    "reqwest",
    "hyper",
    "mio",
    "socket2",
    "async-std",
    "smol",
    "rusqlite",
    "native-tls",
    "openssl",
    "curl",
];

/// Fermeture transitive des dépendances NORMALES, lue dans le graphe
/// RÉSOLU (`Cargo.lock`) — et non dans les manifestes.
///
/// C'était une dette notée au backlog le 26/07 : le contrôle des
/// manifestes ne voit que ce qui est DÉCLARÉ. Une dépendance d'E/S
/// arrivant par transitivité — `kollega-core` → une crate anodine → tokio —
/// serait passée inaperçue, et l'invariant 11 aurait été « prouvé » par un
/// test aveugle à ce chemin.
///
/// Les dev-dependencies sont hors sujet ici : elles n'entrent pas dans
/// l'artefact livré (`proptest` est pur, mais ses propres dépendances ne
/// regardent pas le domaine).
#[test]
fn core_transitive_closure_contains_no_io_crate() {
    let root = crates_dir().parent().expect("racine").to_path_buf();
    let lock: toml::Value = fs::read_to_string(root.join("Cargo.lock"))
        .expect("Cargo.lock doit exister")
        .parse()
        .expect("Cargo.lock illisible");

    let packages = lock["package"].as_array().expect("liste des paquets");
    let mut deps_of: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for package in packages {
        let name = package["name"].as_str().expect("nom de paquet").to_owned();
        let deps = package
            .get("dependencies")
            .and_then(|d| d.as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|d| d.as_str())
                    // Une entrée peut valoir « nom version (source) ».
                    .map(|d| d.split_whitespace().next().unwrap_or(d).to_owned())
                    .collect()
            })
            .unwrap_or_default();
        deps_of.insert(name, deps);
    }

    // Point de départ : les dépendances NORMALES déclarées par le domaine.
    let manifest: toml::Value = fs::read_to_string(crates_dir().join("kollega-core/Cargo.toml"))
        .expect("manifeste du domaine")
        .parse()
        .expect("TOML invalide");
    let mut queue: Vec<String> = dependency_names(&manifest, "dependencies")
        .into_iter()
        .collect();
    assert!(!queue.is_empty(), "extraction suspecte : aucune dépendance");

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut found_io: Vec<String> = Vec::new();
    while let Some(name) = queue.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        if IO_CRATES.contains(&name.as_str()) {
            found_io.push(name.clone());
        }
        if let Some(children) = deps_of.get(&name) {
            queue.extend(children.iter().cloned());
        }
    }
    assert!(
        found_io.is_empty(),
        "invariant 11 violé par TRANSITIVITÉ : kollega-core atteint {found_io:?} \
         à travers le graphe résolu, alors que son manifeste paraît propre"
    );
    assert!(
        seen.len() > 5,
        "fermeture suspecte : seulement {} crates atteintes",
        seen.len()
    );
}

/// Le domaine ne tire jamais d'entropie lui-même.
///
/// `uuid` v4 est activé par la PÉRIPHÉRIE, et l'unification des features de
/// cargo le rend visible partout : le compilateur ne défendrait donc pas
/// cette frontière. Un identifiant fabriqué dans le domaine le rendrait
/// non déterministe — et casserait la reprise, qui repose sur des valeurs
/// dérivables.
#[test]
fn the_domain_never_draws_randomness() {
    let mut sources = Vec::new();
    let core = crates_dir().join("kollega-core/src");
    fn collect(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    collect(&core, &mut sources);
    assert!(!sources.is_empty(), "balayage suspect");

    for path in &sources {
        let content = fs::read_to_string(path).expect("lecture");
        for forbidden in ["new_v4", "getrandom", "thread_rng", "SystemTime::now"] {
            assert!(
                !content.contains(forbidden),
                "{} appelle {forbidden} : le domaine doit rester DÉTERMINISTE \
                 (la reprise en dépend)",
                path.display()
            );
        }
    }
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
                    "invariant 11 violé : kollega-core dépend de {dep} \
                     (autorisées : {CORE_EXTERNAL_ALLOWED:?})"
                );
            }
            for section in ["dev-dependencies", "build-dependencies"] {
                for dep in dependency_names(&manifest, section) {
                    assert!(
                        CORE_DEV_ALLOWED.contains(&dep.as_str()),
                        "invariant 11 violé : kollega-core dépend de {dep} en {section} \
                         (autorisées : {CORE_DEV_ALLOWED:?})"
                    );
                }
            }
        }
    }
    assert_eq!(seen, 10, "le workspace doit contenir exactement 10 crates");
}
