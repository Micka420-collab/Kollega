//! Garde textuelle sur la pose du contexte d'organisation (invariant 1).
//!
//! La seule forme admise pour poser `app.current_org` est
//! `set_config('app.current_org', $1, true)` — paramétrable, et locale à la
//! transaction. Ce test balaie les sources Rust et SQL du dépôt et ÉCHOUE si :
//! - un `SET` SQL sans `LOCAL` (re)apparaît (un `SET` de session survivrait
//!   au retour de la connexion dans le pool : fuite de contexte entre
//!   tenants) ;
//! - la variante `set_config(..., false)` (portée session) apparaît ;
//! - la forme canonique disparaît du point de passage unique.
//!
//! Test purement textuel : il s'exécute sans base et reste une vraie
//! vérification locale.

use std::fs;
use std::path::{Path, PathBuf};

/// Fichiers exclus du balayage : ce test lui-même (il contient les motifs
/// interdits sous forme de données).
const SELF_NAME: &str = "sql_context_guard.rs";

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
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext, "rs" | "sql")
                && path.file_name().and_then(|n| n.to_str()) != Some(SELF_NAME)
            {
                out.push(path);
            }
        }
    }
}

/// Occurrences de `SET <mot>` où `<mot>` n'est pas `LOCAL`, en ignorant
/// `set_config`, `OFFSET`, `RESET`, etc. (frontière de mot à gauche, blanc
/// obligatoire à droite).
fn set_without_local(haystack: &str) -> bool {
    let upper = haystack.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let mut from = 0;
    while let Some(pos) = upper[from..].find("SET") {
        let start = from + pos;
        from = start + 3;
        // Frontière gauche : début de fichier ou caractère non alphanumérique.
        if start > 0 {
            let prev = bytes[start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                continue;
            }
        }
        // Droite : un blanc obligatoire (écarte set_config, SETTING…).
        let after = &upper[start + 3..];
        if !after.starts_with(' ') && !after.starts_with('\t') && !after.starts_with('\n') {
            continue;
        }
        let next_word: String = after
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if next_word != "LOCAL" {
            return true;
        }
    }
    false
}

#[test]
fn context_is_only_set_via_local_parameterized_set_config() {
    let root = workspace_root();
    let mut sources = Vec::new();
    collect_sources(&root.join("crates"), &mut sources);
    collect_sources(&root.join("migrations"), &mut sources);
    assert!(
        sources.len() >= 10,
        "balayage suspect : seulement {} fichiers trouvés",
        sources.len()
    );

    // Motifs interdits, construits par morceaux pour ne pas se piéger
    // soi-même dans d'autres fichiers de test.
    let session_set_config = format!("'app.current_org', $1, {}", "false");
    let raw_set_guc = format!("{} app.", "SET");

    let mut violations = Vec::new();
    for path in &sources {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()));
        if content.contains(&session_set_config) {
            violations.push(format!("{} : set_config en portée session", path.display()));
        }
        if content
            .to_ascii_uppercase()
            .contains(&raw_set_guc.to_ascii_uppercase())
        {
            violations.push(format!("{} : SET brut sur app.*", path.display()));
        }
        if set_without_local(&content) {
            violations.push(format!("{} : SET sans LOCAL", path.display()));
        }
    }
    assert!(
        violations.is_empty(),
        "formes interdites de pose de contexte :\n{}",
        violations.join("\n")
    );

    // La forme canonique doit exister dans le point de passage unique.
    let store = fs::read_to_string(root.join("crates/kollega-store/src/lib.rs"))
        .expect("lecture de kollega-store/src/lib.rs");
    let canonical = format!("set_config('app.current_org', $1, {})", "true");
    assert!(
        store.contains(&canonical),
        "la forme canonique {canonical:?} a disparu de kollega-store"
    );
}
