//! Garde : chaque migration est réversible, OU dit pourquoi elle ne l'est
//! pas — la lettre exacte de l'invariant 13.
//!
//! Ce que le job CI `reversibilite` prouve : que les `.down.sql` EXISTANTS
//! ramènent bien à l'état vierge. Ce qu'il ne prouve pas : qu'une
//! migration en ait un. La nuance a été vérifiée, pas supposée :
//!
//! - `sqlx::migrate!` **accepte** une migration sans `.down.sql`, même
//!   après recompilation forcée du crate — il n'y a donc aucune protection
//!   à la compilation, contrairement à ce qu'on pourrait espérer ;
//! - le job de réversibilité rattraperait le cas le plus courant, parce
//!   qu'une table restée debout après la descente ferait diverger le
//!   `pg_dump`. Mais il compare `pg_dump --schema-only`, plus les rôles,
//!   les extensions et l'ACL : **une migration qui ne touche que des
//!   DONNÉES** — un remplissage, une correction de lignes — n'y laisse
//!   aucune trace. Elle passerait verte sans descente, et l'invariant 13
//!   serait violé en silence tout en restant marqué « prouvé ».
//!
//! Cette garde ferme l'écart et, accessoirement, rend enfin utilisable la
//! seconde branche de l'invariant : une irréversibilité ASSUMÉE et écrite
//! dans le fichier. Sans mécanisme, cette branche n'était qu'une phrase.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

/// Marqueur d'irréversibilité assumée, à porter dans le `.up.sql`.
const IRREVERSIBLE: &str = "IRRÉVERSIBLE";

/// Longueur minimale de la justification qui suit le marqueur.
///
/// Sans ce plancher, le marqueur deviendrait un laissez-passer : on
/// écrirait le mot pour faire taire la garde. L'invariant demande une
/// justification, pas une incantation.
const MIN_JUSTIFICATION: usize = 60;

fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("racine du dépôt")
        .join("migrations")
}

/// `0003_tasks_audit_credits.up.sql` → (`0003`, `tasks_audit_credits`).
fn split_stem(file_name: &str, suffix: &str) -> Option<(String, String)> {
    let stem = file_name.strip_suffix(suffix)?;
    let (version, name) = stem.split_once('_')?;
    Some((version.to_owned(), name.to_owned()))
}

#[test]
fn every_migration_is_reversible_or_says_why_not() {
    let dir = migrations_dir();
    let mut ups: BTreeMap<String, (String, PathBuf)> = BTreeMap::new();
    let mut downs: BTreeSet<String> = BTreeSet::new();
    let mut duplicates: Vec<String> = Vec::new();

    for entry in fs::read_dir(&dir).expect("lecture de migrations/") {
        let path = entry.expect("entrée lisible").path();
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some((version, name)) = split_stem(file_name, ".up.sql") {
            if let Some((previous, _)) = ups.insert(version.clone(), (name, path)) {
                duplicates.push(format!("{version} : {previous} et un autre"));
            }
        } else if let Some((version, _)) = split_stem(file_name, ".down.sql") {
            downs.insert(version);
        }
    }

    assert!(
        ups.len() >= 6,
        "balayage suspect : {} migrations trouvées dans {}",
        ups.len(),
        dir.display()
    );
    assert!(
        duplicates.is_empty(),
        "deux migrations portent le MÊME numéro de version : {duplicates:?}. \
         sqlx applique les migrations par version — deux homonymes rendent \
         l'ordre, et donc l'état de la base, dépendant du système de \
         fichiers."
    );

    let mut faults: Vec<String> = Vec::new();
    for (version, (name, path)) in &ups {
        if downs.contains(version) {
            continue;
        }
        let content = fs::read_to_string(path).expect("lecture d'une migration");
        let justification = content.split_once(IRREVERSIBLE).map(|(_, rest)| {
            // La justification est le BLOC DE COMMENTAIRE qui suit le
            // marqueur, pas le reste du fichier. Compter tout ce qui suit
            // laissait le SQL lui-même faire office de justification : dix
            // lignes de DDL et le marqueur passait, ce qui vidait la règle
            // de son sens. Défaut trouvé en relisant cette garde.
            let mut lines = rest.lines();
            let mut counted: usize = lines
                .next()
                .map(|first| first.chars().filter(|c| !c.is_whitespace()).count())
                .unwrap_or(0);
            for line in lines {
                let trimmed = line.trim_start();
                if !trimmed.starts_with("--") {
                    break;
                }
                counted += trimmed
                    .trim_start_matches('-')
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .count();
            }
            counted
        });
        match justification {
            Some(len) if len >= MIN_JUSTIFICATION => {}
            Some(len) => faults.push(format!(
                "{version}_{name} : irréversibilité marquée mais justifiée en \
                 {len} caractères seulement (minimum {MIN_JUSTIFICATION})"
            )),
            None => faults.push(format!(
                "{version}_{name} : aucun .down.sql et aucune justification"
            )),
        }
    }
    assert!(
        faults.is_empty(),
        "invariant 13 — une migration doit être réversible OU justifier de \
         ne pas l'être DANS le fichier (marqueur « {IRREVERSIBLE} » suivi de \
         la raison). Ni `sqlx::migrate!`, qui accepte une migration sans \
         descente, ni le job CI de réversibilité, qui ne compare que le \
         SCHÉMA et laisserait passer une migration de données, ne \
         l'attrapent. Manquements : {faults:?}"
    );

    let orphan_downs: Vec<&String> = downs.iter().filter(|v| !ups.contains_key(*v)).collect();
    assert!(
        orphan_downs.is_empty(),
        "descente(s) sans montée correspondante : {orphan_downs:?}. Le job de \
         réversibilité les jouerait quand même — une descente qui défait \
         quelque chose que personne n'a fait est au mieux morte, au pire \
         destructrice."
    );
}
