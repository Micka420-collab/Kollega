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

/// v2 — Occurrences DANGEREUSES de `SET <mot>` sans `LOCAL`.
///
/// La v1 bannissait tout `SET <mot>` hors `LOCAL` : elle ne pouvait pas
/// survivre au premier `UPDATE … SET colonne = …` légitime — prédit en
/// revue, advenu à la tranche verticale. La menace réelle de l'invariant 1
/// est le changement de configuration ou d'identité à portée SESSION (il
/// survivrait au retour de la connexion dans le pool) : sont interdits,
/// sans `LOCAL` — `SET <guc.pointée>` (dont `app.current_org`),
/// `SET SESSION …`, `SET ROLE …`, `SET search_path …`. Les formes SQL
/// d'affectation (`UPDATE … SET col`, `ALTER TABLE … SET NOT NULL`,
/// `SET DEFAULT`) ne touchent pas la session : autorisées.
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
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
            .collect();
        let dangerous = next_word != "LOCAL"
            && (next_word.contains('.')
                || next_word == "SESSION"
                || next_word == "ROLE"
                || next_word == "SEARCH_PATH");
        if dangerous {
            return true;
        }
    }
    false
}

#[test]
fn guard_distinguishes_session_set_from_sql_assignment() {
    // Dangereux : configuration ou identité de session, sans LOCAL.
    for hit in [
        "SET app.current_org = 'x'",
        "set session characteristics as transaction",
        "SET ROLE kollega_migrate",
        "SET search_path TO public",
        "SET myext.tenant = $1",
    ] {
        assert!(set_without_local(hit), "aurait dû être signalé : {hit}");
    }
    // Légitime : affectations SQL et formes LOCAL.
    for ok in [
        "UPDATE tasks SET state = $2, updated_at = now()",
        "UPDATE credits SET balance_cents = $2",
        "ALTER TABLE users ALTER COLUMN email SET NOT NULL",
        "ALTER COLUMN x SET DEFAULT 0",
        "SET LOCAL app.current_org = 'x'",
        "OFFSET 3",
        "set_config('a.b', $1, true)",
    ] {
        assert!(!set_without_local(ok), "faux positif : {ok}");
    }
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
    let raw_set_guc = format!("{} app.", "SET");
    // La SEULE forme admise, littéralement. Tout autre appel de
    // `set_config` portant sur `app.current_org` est refusé, quelle que
    // soit son écriture.
    //
    // La v2 de cette garde ne cherchait qu'une chaîne : `$1, false` en
    // minuscules. Trois écritures équivalentes passaient donc au travers —
    // `FALSE`, `'f'`, et un troisième argument paramétré `$2` — et chacune
    // pose le contexte en portée SESSION. Un contexte de session survit au
    // retour de la connexion dans le pool : la requête suivante, celle
    // d'une AUTRE organisation, s'exécuterait sous le contexte de la
    // précédente. C'est exactement la catastrophe que l'invariant 1
    // existe pour empêcher, et la garde la laissait passer par trois
    // portes. Interdire tout ce qui n'est pas la forme canonique ferme la
    // question au lieu d'énumérer les fautes, toujours incomplète.
    let canonical_call = format!("set_config('app.current_org', $1, {})", "true");

    let mut violations = Vec::new();
    for path in &sources {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()));
        let mut rest = content.as_str();
        while let Some(at) = rest.find("set_config(") {
            let call = &rest[at..];
            rest = &call["set_config(".len()..];
            // Fenêtre ramenée à une frontière de caractère : les
            // commentaires sont en français.
            let mut end = call.len().min(80);
            while !call.is_char_boundary(end) {
                end -= 1;
            }
            if call[..end].contains("app.current_org") && !call.starts_with(&canonical_call) {
                violations.push(format!(
                    "{} : appel de set_config sur app.current_org qui n'est PAS \
                     la forme canonique {canonical_call:?} — toute autre écriture \
                     risque la portée session",
                    path.display()
                ));
            }
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
