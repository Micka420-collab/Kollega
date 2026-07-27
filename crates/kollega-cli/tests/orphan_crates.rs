//! Garde : l'inventaire des crates ÉCRITES MAIS BRANCHÉES NULLE PART est
//! tenu à jour, dans les deux sens.
//!
//! Une crate que personne n'utilise n'est pas du produit : c'est du code
//! qui compile, dont les tests passent, et qu'aucun chemin d'exécution
//! n'atteint. `kollega-model` en est l'exemple vif — 273 lignes de contrat
//! réel (clé expurgée, quatre modes d'échec, facturation en jetons), et
//! zéro dépendant. Ses tests verts ne prouvent donc rien du produit.
//!
//! C'est précisément le genre d'écart qui fait qu'un README survend : on
//! écrit « le contrat de modèle existe », c'est vrai, et le lecteur
//! comprend « le modèle est branché », ce qui est faux. La règle du dépôt
//! est l'avancement RÉEL ; cette garde la rend mécanique plutôt que
//! vertueuse.
//!
//! Elle échoue dans les DEUX sens, et c'est le point :
//! - une NOUVELLE crate orpheline apparaît → elle n'est pas dans la liste
//!   → rouge, il faut la brancher ou l'assumer par écrit ;
//! - une crate orpheline se fait BRANCHER → elle reste dans la liste alors
//!   qu'elle n'est plus orpheline → rouge, ce qui force à revisiter le
//!   README et la matrice au moment même où l'état change.
//!
//! Sans le second sens, la liste deviendrait une archive périmée — la
//! forme de mensonge la plus courante dans un dépôt.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// Crates du workspace qu'AUCUNE autre n'utilise, au 29/07/2026.
///
/// - `kollega-model` : le contrat `ModelProvider` (requête à prompt
///   STRUCTURÉ, estimation de jetons, quatre modes d'échec, `ApiKey`
///   expurgée) est écrit et testé, mais la boucle ne l'appelle pas — elle
///   passe par son propre port, qui ne transporte qu'un numéro
///   d'itération. Deux conséquences à ne pas perdre de vue : l'invariant 7
///   n'a, en aval de l'assemblage, aucun chemin réel à protéger ; et
///   l'invariant 5 ne peut pas devenir « vérifié AVANT » tant que la
///   boucle ne reçoit pas l'estimation que `ModelRequest` porte déjà.
/// - `kollega-memory`, `kollega-tools` : squelettes de 9 lignes (M5, M2).
const KNOWN_ORPHANS: &[&str] = &["kollega-memory", "kollega-model", "kollega-tools"];

/// `kollega-cli` produit le binaire : n'être utilisé par personne est sa
/// place normale dans le graphe, pas un oubli de câblage.
const ENTRY_POINT: &str = "kollega-cli";

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ doit exister")
        .to_path_buf()
}

/// Noms de dépendances de TOUTES les sections, y compris `[target.*]` —
/// une crate utilisée seulement en dev ou sous une cible reste utilisée.
fn all_dependency_names(manifest: &toml::Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut harvest = |table: Option<&toml::Value>| {
        if let Some(t) = table.and_then(|d| d.as_table()) {
            out.extend(t.keys().cloned());
        }
    };
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        harvest(manifest.get(section));
        if let Some(targets) = manifest.get("target").and_then(|t| t.as_table()) {
            for target_cfg in targets.values() {
                harvest(target_cfg.get(section));
            }
        }
    }
    out
}

/// Concatène le texte de tous les `.rs` d'un répertoire, récursivement.
fn append_rust_sources(dir: &std::path::Path, out: &mut String) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            append_rust_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(text) = fs::read_to_string(&path) {
                out.push_str(&text);
            }
        }
    }
}

#[test]
fn the_inventory_of_unwired_crates_is_exact() {
    let mut members: BTreeSet<String> = BTreeSet::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut dead_edges: Vec<String> = Vec::new();

    for entry in fs::read_dir(crates_dir()).expect("lecture de crates/") {
        let path = entry.expect("entrée lisible").path().join("Cargo.toml");
        let manifest: toml::Value = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()))
            .parse()
            .unwrap_or_else(|e| panic!("TOML invalide dans {} : {e}", path.display()));
        let name = manifest["package"]["name"]
            .as_str()
            .expect("package.name présent")
            .to_owned();
        // Une arête de manifeste ne prouve pas un USAGE. Sans cette
        // vérification, il suffisait d'ajouter une ligne de dépendance
        // sans écrire une seule ligne de code pour qu'une crate cesse
        // d'être « orpheline » aux yeux de la garde — et le README aurait
        // alors annoncé branché ce qui ne l'est pas. C'est précisément la
        // fausse affirmation que cette garde existe pour rendre
        // impossible.
        let crate_dir = path.parent().expect("répertoire de la crate");
        let mut crate_sources = String::new();
        for sub in ["src", "tests", "benches", "examples"] {
            append_rust_sources(&crate_dir.join(sub), &mut crate_sources);
        }
        for dep in all_dependency_names(&manifest)
            .into_iter()
            .filter(|d| d.starts_with("kollega-"))
        {
            if crate_sources.contains(&dep.replace('-', "_")) {
                used.insert(dep);
            } else {
                dead_edges.push(format!("{name} déclare {dep} sans l'utiliser"));
            }
        }
        members.insert(name);
    }
    assert!(
        dead_edges.is_empty(),
        "dépendance(s) DÉCLARÉE(S) mais jamais employée(s) : {dead_edges:?}. \
         Une arête morte fait passer une crate pour branchée alors qu'aucun \
         code ne l'atteint — exactement ce que l'inventaire ci-dessous doit \
         empêcher d'affirmer."
    );
    assert!(
        members.len() >= 10,
        "balayage suspect : {} crates trouvées",
        members.len()
    );

    let orphans: BTreeSet<&str> = members
        .iter()
        .map(String::as_str)
        .filter(|name| *name != ENTRY_POINT && !used.contains(*name))
        .collect();
    let declared: BTreeSet<&str> = KNOWN_ORPHANS.iter().copied().collect();

    let appeared: Vec<&&str> = orphans.difference(&declared).collect();
    assert!(
        appeared.is_empty(),
        "crate(s) écrite(s) mais branchée(s) NULLE PART, et non déclarée(s) : \
         {appeared:?}. Du code qui compile et que rien n'atteint n'est pas du \
         produit — le brancher, ou l'inscrire dans KNOWN_ORPHANS en disant \
         pourquoi, pour que le README ne laisse pas croire qu'il sert."
    );

    let wired: Vec<&&str> = declared.difference(&orphans).collect();
    assert!(
        wired.is_empty(),
        "{wired:?} n'est plus orpheline : quelqu'un l'a branchée. Retirer \
         l'entrée de KNOWN_ORPHANS — et profiter de ce rouge pour remettre \
         README.md et docs/matrice-invariants.md d'accord avec l'état réel, \
         c'est exactement le moment où ils dérivent."
    );
}
