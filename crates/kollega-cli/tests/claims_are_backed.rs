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
/// Le seuil était à « plus de six caractères ET contenant un souligné ».
/// Deux exigences de trop : la matrice cite « tests `anchor` » pour
/// l'invariant 4, six lettres sans souligné — la citation était donc
/// ignorée en silence, et aurait survécu à la disparition de ce qu'elle
/// nomme. Une garde d'honnêteté documentaire qui écarte discrètement ce
/// qu'elle ne sait pas classer perd sa raison d'être. Restent écartés les
/// chemins et fichiers (point, tiret, barre oblique), qui ne sont pas des
/// noms d'identifiants.
fn looks_like_a_test_name(candidate: &str) -> bool {
    candidate.len() >= 4
        && candidate
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Le numéro d'un ADR, dans son titre, est celui de son fichier.
///
/// Deux ADR portaient un titre faux — `0003` s'annonçait « ADR 0002 », et
/// `0004` « ADR 0001 » : les fichiers avaient été renumérotés au pivot sans
/// que leurs en-têtes suivent. Ce n'est pas cosmétique. Les ADR se citent
/// entre eux par numéro, et un document qui se présente sous le numéro d'un
/// autre envoie lire la mauvaise décision — au moment précis où quelqu'un
/// cherche pourquoi le système est fait ainsi.
#[test]
fn every_adr_title_matches_its_file_number() {
    let dir = repo_root().join("docs/adr");
    let mut checked = 0;
    let mut wrong = Vec::new();
    for entry in fs::read_dir(&dir).expect("lecture de docs/adr/").flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(number) = name.split('-').next().filter(|n| n.len() == 4) else {
            continue;
        };
        let text = fs::read_to_string(&path).expect("lecture d'un ADR");
        let title = text.lines().next().unwrap_or_default();
        checked += 1;
        // « ADR-0004 » ou « ADR 0004 » : la forme du séparateur importe peu,
        // le numéro non.
        if !title.contains(number) {
            wrong.push(format!("{name} : titre « {title} »"));
        }
    }
    assert!(checked >= 7, "balayage suspect : {checked} ADR trouvés");
    assert!(
        wrong.is_empty(),
        "ADR dont le titre annonce un autre numéro que son fichier : \
         {wrong:?}. Les ADR se citent entre eux par numéro — un titre faux \
         envoie lire la mauvaise décision."
    );
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
        // La matrice contient PLUSIEURS tableaux à lignes numérotées (celui
        // des preuves, celui d'« où vit l'invariant », celui des
        // sabotages). Seul le premier a une colonne « Test » en 3e
        // position ; les autres y mettent autre chose, et les lire ferait
        // prendre un mot comme `compile_fail` pour une fonction disparue.
        // On les distingue par leur LARGEUR : le tableau des preuves a six
        // colonnes, les autres deux ou trois.
        if cells.len() < 5 || cells[0].trim().parse::<u32>().is_err() {
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

    // La matrice cite aussi des JOBS de CI — la preuve entière de
    // l'invariant 13 en est un. Rien ne vérifiait leur existence : renommer
    // `reversibilite` dans le workflow aurait laissé la matrice affirmer
    // une preuve devenue introuvable. Un adossement valide est donc une
    // fonction, une suite, ou un job.
    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect(".github/workflows/ci.yml doit exister");
    let declares_job = |name: &str| workflow.contains(&format!("\n  {name}:"));

    let missing: Vec<&String> = claimed
        .iter()
        .filter(|name| {
            !declares_function(&corpus, name) && !suites.contains(*name) && !declares_job(name)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "la matrice invoque des tests qui N'EXISTENT PLUS — elle affirmerait \
         une preuve disparue, et le README avec elle : {missing:?}"
    );
}
