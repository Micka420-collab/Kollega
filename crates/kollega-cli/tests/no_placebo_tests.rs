//! Garde : aucun test au corps vide.
//!
//! Le dépôt a hébergé cinq `fn crate_compiles() {}` — un par crate, sous
//! `#[cfg(test)]`. Chacun comptait comme un test vert et ne prouvait rien
//! de plus que le compilateur, qui compile la crate de toute façon. Ils
//! gonflaient le nombre affiché au README, et ce nombre est l'un des rares
//! chiffres que le lecteur retient.
//!
//! Un corps vide n'est jamais une vérification : un test qui n'assère rien
//! peut encore avoir du sens s'il panique (un `unwrap` qui doit passer, un
//! chemin qui ne doit pas diverger), mais `{}` ne s'exécute pas, ne
//! compare rien, et ne peut pas échouer. C'est la seule forme dont on
//! puisse dire mécaniquement qu'elle ne teste rien — et donc la seule
//! qu'il soit honnête d'interdire par une garde.
//!
//! Portée volontairement étroite : la garde ne juge PAS de la qualité d'un
//! test non vide. Prétendre le faire textuellement produirait des faux
//! positifs, et une garde qui crie à tort finit désactivée.

use std::fs;
use std::path::{Path, PathBuf};

/// Ce fichier cite le motif interdit dans sa documentation.
const SELF_NAME: &str = "no_placebo_tests.rs";

/// Attributs qui font d'une fonction un test exécuté par `cargo test`.
const TEST_ATTRIBUTES: &[&str] = &["#[test]", "#[tokio::test]"];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("racine du dépôt")
        .to_path_buf()
}

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "target" && name != ".git" {
                collect_sources(&path, out);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && path.file_name().and_then(|n| n.to_str()) != Some(SELF_NAME)
        {
            out.push(path);
        }
    }
}

#[test]
fn no_test_has_an_empty_body() {
    let root = workspace_root();
    let mut sources = Vec::new();
    collect_sources(&root.join("crates"), &mut sources);
    assert!(
        sources.len() >= 10,
        "balayage suspect : {} fichiers trouvés",
        sources.len()
    );

    let mut placebos = Vec::new();
    let mut tests_seen = 0_usize;
    for path in &sources {
        let content = fs::read_to_string(path).expect("lecture d'une source");
        // On garde la trace du dernier attribut de test rencontré : la
        // fonction qui suit est un test, quelles que soient les lignes
        // d'attributs ou de commentaires intercalées.
        let mut pending_test = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if TEST_ATTRIBUTES.contains(&trimmed) {
                pending_test = true;
                tests_seen += 1;
                continue;
            }
            if !pending_test || trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with("fn ") || trimmed.starts_with("async fn ") {
                if trimmed.ends_with("() {}") {
                    placebos.push(format!("{} : {trimmed}", path.display()));
                }
                pending_test = false;
            } else if !trimmed.starts_with('#') {
                // Autre chose qu'un attribut ou une signature : l'attribut
                // ne portait pas sur une fonction, on ne conclut rien.
                pending_test = false;
            }
        }
    }

    assert!(
        tests_seen >= 50,
        "balayage suspect : seulement {tests_seen} tests repérés dans le \
         dépôt — la reconnaissance des attributs est cassée, et cette garde \
         ne garde plus rien"
    );
    assert!(
        placebos.is_empty(),
        "test(s) au corps VIDE : ils comptent comme des verts et ne \
         prouvent rien de plus que le compilateur, qui compile la crate de \
         toute façon. Les gonfler dans le nombre affiché au README, c'est \
         le survendre. Écrire une vraie vérification, ou supprimer : \
         {placebos:?}"
    );
}
