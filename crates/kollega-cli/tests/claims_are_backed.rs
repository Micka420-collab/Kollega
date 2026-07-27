//! La matrice des invariants ne peut pas mentir en silence.
//!
//! `docs/matrice-invariants.md` nomme, pour chaque invariant, les tests qui
//! le soutiennent. C'est le document de vérité du projet — et jusqu'ici,
//! rien ne le reliait au code : un test renommé ou supprimé aurait laissé
//! la matrice (et le README qui la résume) affirmer une preuve disparue,
//! **sans que rien ne le signale**. C'était l'inquiétude n°1 du rapport de
//! la nuit du 28 au 29/07 ; ce test la ferme.
//!
//! Principe : chaque nom de test cité dans la colonne « Test » doit exister
//! comme fonction dans les sources. Purement textuel, exécutable sans base.

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

/// Vrai si `corpus` déclare une fonction EXACTEMENT nommée `name`.
///
/// La recherche naïve par sous-chaîne ne suffisait pas : renommer
/// `fn essai` en `fn essai_bis` la laissait passer, puisque le texte
/// contient toujours `fn essai`. Un test renommé serait donc resté
/// « adossé » sans exister — défaut trouvé en sabotant cette garde même.
/// D'où la frontière de mot : ce qui suit le nom ne doit pas prolonger un
/// identifiant.
fn declares_function(corpus: &str, name: &str) -> bool {
    let needle = format!("fn {name}");
    let mut from = 0;
    while let Some(pos) = corpus[from..].find(&needle) {
        let end = from + pos + needle.len();
        from = end;
        match corpus[end..].chars().next() {
            Some(c) if c.is_alphanumeric() || c == '_' => continue,
            _ => return true,
        }
    }
    false
}

/// Vrai si l'entrée ressemble à un nom de fonction de test Rust — et non à
/// un chemin de fichier, une colonne SQL ou une commande.
fn looks_like_a_test_name(candidate: &str) -> bool {
    !candidate.is_empty()
        && candidate.len() > 6
        && candidate
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && candidate.contains('_')
}

#[test]
fn every_test_named_in_the_matrix_actually_exists() {
    let root = repo_root();
    let matrix = fs::read_to_string(root.join("docs/matrice-invariants.md"))
        .expect("docs/matrice-invariants.md doit exister");

    // Colonne « Test » = 3e cellule des lignes de tableau dont la 1re
    // cellule est un numéro d'invariant.
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    for line in matrix.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed.trim_matches('|').split('|').collect();
        if cells.len() < 3 || cells[0].trim().parse::<u32>().is_err() {
            continue;
        }
        for chunk in cells[2].split('`').skip(1).step_by(2) {
            let candidate = chunk.trim().trim_end_matches("!");
            if looks_like_a_test_name(candidate) {
                claimed.insert(candidate.to_owned());
            }
        }
    }
    assert!(
        claimed.len() >= 10,
        "extraction suspecte : seulement {} tests cités trouvés dans la matrice",
        claimed.len()
    );

    let mut sources = Vec::new();
    collect_rust_sources(&root.join("crates"), &mut sources);
    let corpus: String = sources
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .collect();

    // Un adossement valide est soit une FONCTION de test, soit une SUITE
    // entière (un fichier `tests/<nom>.rs`) : la matrice cite légitimement
    // les deux, et citer une suite reste une preuve tant qu'elle existe.
    let suites: BTreeSet<String> = sources
        .iter()
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()))
        .map(str::to_owned)
        .collect();

    let missing: Vec<&String> = claimed
        .iter()
        .filter(|name| !declares_function(&corpus, name) && !suites.contains(*name))
        .collect();
    assert!(
        missing.is_empty(),
        "la matrice invoque des tests qui N'EXISTENT PLUS — elle affirmerait \
         une preuve disparue, et le README avec elle : {missing:?}"
    );
}
