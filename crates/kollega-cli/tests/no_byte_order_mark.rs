//! Garde : aucun fichier source ne commence par une marque d'ordre d'octets.
//!
//! Le dépôt se travaille sous Windows, et PowerShell écrit par défaut une
//! marque d'ordre d'octets UTF-8 (`EF BB BF`) en tête de fichier —
//! `Set-Content -Encoding utf8` comme `Out-File -Encoding utf8`. Un outil
//! qui réécrit un fichier en passant par là en introduit une sans que rien
//! ne le signale : c'est arrivé le 29/07 sur `.github/workflows/ci.yml`,
//! et la marque a été poussée sur `main` sans que la CI bronche.
//!
//! Ce n'est pas cosmétique. Selon le format, la marque est :
//! - **fatale** dans un `.sql` joué par `psql` : les trois octets font
//!   partie du premier jeton, la migration échoue sur une erreur de
//!   syntaxe incompréhensible — et une migration qui ne passe pas est une
//!   production qui ne démarre pas ;
//! - **hasardeuse** en YAML et en TOML : certains analyseurs l'acceptent,
//!   d'autres non. Dépendre de la tolérance de celui du jour est un pari.
//!
//! GitHub Actions a toléré la nôtre. On ne construit pas une chaîne de
//! preuve sur la tolérance d'un analyseur tiers.

use std::fs;
use std::path::{Path, PathBuf};

/// Marque d'ordre d'octets UTF-8.
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Extensions où la marque est fatale ou hasardeuse.
///
/// Le Markdown en est exclu : la marque y est inoffensive, et interdire
/// au-delà du nécessaire est le meilleur moyen de faire désactiver une
/// garde.
const GUARDED: &[&str] = &["rs", "sql", "toml", "yml", "yaml"];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("racine du dépôt")
        .to_path_buf()
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "target" && name != ".git" {
                collect(&path, out);
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| GUARDED.contains(&ext))
        {
            out.push(path);
        }
    }
}

#[test]
fn no_source_file_starts_with_a_byte_order_mark() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["crates", "migrations", ".github", "tools"] {
        collect(&root.join(dir), &mut files);
    }
    for name in ["Cargo.toml", "rustfmt.toml"] {
        let path = root.join(name);
        if path.is_file() {
            files.push(path);
        }
    }
    assert!(
        files.len() >= 20,
        "balayage suspect : {} fichiers trouvés",
        files.len()
    );

    let marked: Vec<String> = files
        .iter()
        .filter(|path| {
            fs::read(path)
                .map(|bytes| bytes.starts_with(BOM))
                .unwrap_or(false)
        })
        .map(|path| {
            path.strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();

    assert!(
        marked.is_empty(),
        "marque d'ordre d'octets UTF-8 en tête de fichier : {marked:?}. \
         Dans un .sql jouée par psql elle fait échouer la migration sur une \
         erreur de syntaxe illisible ; en YAML et en TOML elle dépend de la \
         tolérance de l'analyseur. Sous PowerShell, écrire avec \
         `[System.IO.File]::WriteAllText(chemin, contenu, \
         (New-Object System.Text.UTF8Encoding($false)))` — `Set-Content \
         -Encoding utf8` et `Out-File -Encoding utf8` en posent une."
    );
}
