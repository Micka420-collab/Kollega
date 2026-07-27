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

/// Vrai si, après l'accolade ouvrante, le corps ne contient RIEN — ni
/// instruction, ni expression, seulement des blancs et des commentaires.
///
/// Écrit ainsi plutôt qu'en cherchant le littéral `() {}` : la première
/// version de cette garde ne reconnaissait que cette forme exacte, et se
/// laissait contourner par un corps vide écrit sur deux lignes ou par
/// `() { }`. `cargo fmt --check`, imposé en CI, ramène en pratique ces
/// formes à `{}` — mais faire dépendre cette garde du comportement de
/// rustfmt était un couplage invisible : il suffisait d'assouplir la règle
/// de format, jugée cosmétique, pour rouvrir le trou sans s'en apercevoir.
fn body_is_empty(after_open_brace: &str) -> bool {
    let mut rest = after_open_brace;
    loop {
        rest = rest.trim_start();
        let Some(stripped) = rest.strip_prefix("//") else {
            return rest.starts_with('}');
        };
        match stripped.find('\n') {
            Some(end) => rest = &stripped[end..],
            None => return false,
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
        for attribute in TEST_ATTRIBUTES {
            let mut from = 0;
            while let Some(at) = content[from..].find(attribute) {
                let start = from + at + attribute.len();
                from = start;
                tests_seen += 1;
                // La signature suit l'attribut, éventuellement après
                // d'autres attributs ou des commentaires. On borne la
                // recherche : au-delà, ce n'est plus la fonction annotée.
                // Fenêtre ramenée à une frontière de caractère : couper
                // à l'octet près traverserait un accent des commentaires
                // français et paniquerait.
                let mut end = content.len().min(start + 400);
                while !content.is_char_boundary(end) {
                    end -= 1;
                }
                let window = &content[start..end];
                let Some(fn_at) = window.find("fn ") else {
                    continue;
                };
                let signature = &window[fn_at..];
                let Some(close) = signature.find(')') else {
                    continue;
                };
                let Some(brace) = signature[close..].find('{') else {
                    continue;
                };
                if body_is_empty(&signature[close + brace + 1..]) {
                    let name: String = signature
                        .chars()
                        .take_while(|c| *c != '(')
                        .collect::<String>()
                        .trim()
                        .to_owned();
                    placebos.push(format!("{} : {name}()", path.display()));
                }
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
