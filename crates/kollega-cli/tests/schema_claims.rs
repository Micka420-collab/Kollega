//! Garde : la documentation ne peut pas nommer une colonne qui n'existe pas.
//!
//! Née d'une trouvaille du 29/07 : `docs/credits-concurrence.md` faisait de
//! `tasks.cost_cents` la pièce centrale de deux de ses exigences, et cette
//! colonne **n'a jamais existé**. Le consommé d'une tâche vit dans
//! l'enveloppe `tasks.state`. Personne ne l'avait vu, parce que rien ne
//! reliait la prose au schéma.
//!
//! Ce n'est pas une coquille : un document de conception est ce qu'on suit
//! quand on reprend le travail six mois plus tard. Nommer une colonne
//! absente envoie chercher ce qui n'est pas là, ou pire — invite à
//! l'ajouter en croyant réparer un oubli, alors que la conception réelle a
//! délibérément mis l'information ailleurs.
//!
//! Portée étroite, à dessein : seuls sont vérifiés les jetons de la forme
//! `table.colonne` dont le préfixe est une table connue du schéma. Une
//! prose qui parle de `app.current_org` ou de `Cargo.toml` n'est pas
//! concernée — et une garde qui déborde de son objet finit désactivée.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Mots-clés qui ouvrent une contrainte, non une colonne.
const NOT_A_COLUMN: &[&str] = &[
    "PRIMARY",
    "FOREIGN",
    "UNIQUE",
    "CHECK",
    "CONSTRAINT",
    "EXCLUDE",
    "LIKE",
];

/// Tables PRÉVUES mais non construites, que la documentation a le droit de
/// nommer — avec leur jalon, pour qu'un lecteur sache qu'il ne les
/// trouvera pas dans le schéma d'aujourd'hui.
const PLANNED_TABLES: &[&str] = &[
    // ADR-0005 — authentification hors contexte, jalon M1.
    "login_identities",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("racine du dépôt")
        .to_path_buf()
}

/// Le schéma tel que les migrations le construisent : table → colonnes.
fn schema_from_migrations(root: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut schema: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut files: Vec<PathBuf> = fs::read_dir(root.join("migrations"))
        .expect("lecture de migrations/")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".up.sql"))
        .collect();
    // Ordre des versions : une colonne ajoutée par 0004 doit être vue.
    files.sort();

    for path in files {
        let sql = fs::read_to_string(&path).expect("lecture d'une migration");
        let mut current: Option<String> = None;
        for line in sql.lines() {
            let trimmed = line.trim();
            let upper = trimmed.to_ascii_uppercase();

            if let Some(rest) = upper.strip_prefix("CREATE TABLE ") {
                let name = rest
                    .trim_start_matches("IF NOT EXISTS ")
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect::<String>()
                    .to_ascii_lowercase();
                schema.entry(name.clone()).or_default();
                current = Some(name);
                continue;
            }
            if let Some(rest) = upper.strip_prefix("ALTER TABLE ") {
                let name = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect::<String>()
                    .to_ascii_lowercase();
                if let Some(at) = upper.find("ADD COLUMN ") {
                    let column: String = upper[at + "ADD COLUMN ".len()..]
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    schema
                        .entry(name)
                        .or_default()
                        .insert(column.to_ascii_lowercase());
                }
                continue;
            }
            let Some(table) = current.as_ref() else {
                continue;
            };
            if trimmed.starts_with(')') || trimmed.is_empty() || trimmed.starts_with("--") {
                if trimmed.starts_with(')') {
                    current = None;
                }
                continue;
            }
            let first: String = trimmed
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if first.is_empty() || NOT_A_COLUMN.contains(&first.to_ascii_uppercase().as_str()) {
                continue;
            }
            schema
                .entry(table.clone())
                .or_default()
                .insert(first.to_ascii_lowercase());
        }
    }
    schema
}

/// Jetons entre accents graves, de la forme `mot.mot`.
fn backticked_dotted_pairs(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::trim)
        .filter(|token| {
            (1..=2).contains(&token.chars().filter(|c| *c == '.').count())
                && token
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn no_document_names_a_column_that_does_not_exist() {
    let root = repo_root();
    let schema = schema_from_migrations(&root);
    assert!(
        schema.len() >= 6,
        "extraction du schéma suspecte : {} tables trouvées",
        schema.len()
    );
    assert!(
        schema
            .get("credits")
            .is_some_and(|c| c.contains("balance_cents")),
        "extraction du schéma cassée : credits.balance_cents introuvable \
         alors que la migration 0003 le crée"
    );

    let mut documents: Vec<PathBuf> = fs::read_dir(root.join("docs"))
        .expect("lecture de docs/")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    documents.push(root.join("README.md"));

    let mut wrong = Vec::new();
    for path in &documents {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for token in backticked_dotted_pairs(&text) {
            // Forme QUALIFIÉE `public.tasks.state` : on ne garde que les
            // deux derniers segments. Sans cela elle échappait au contrôle,
            // la découpe exigeant un point unique — évasion trouvée en
            // attaquant cette garde plutôt qu'en la confirmant.
            let token = token
                .rsplit_once('.')
                .map(|(prefix, column)| {
                    let table = prefix.rsplit_once('.').map_or(prefix, |(_, t)| t);
                    format!("{table}.{column}")
                })
                .unwrap_or(token);
            let (table, column) = token.split_once('.').expect("un point");
            let Some(columns) = schema.get(table) else {
                continue; // pas une table : hors sujet.
            };
            if !columns.contains(column) {
                wrong.push(format!(
                    "{} cite {token} — la table existe, pas la colonne (colonnes : {:?})",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    columns
                ));
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "la documentation nomme une colonne absente du schéma. Un document \
         de conception est ce qu'on suit en reprenant le travail : nommer \
         une colonne qui n'existe pas envoie chercher ce qui n'est pas là, \
         ou invite à l'ajouter en croyant réparer un oubli. {wrong:?}"
    );

    // Deuxième passe : les TABLES nommées explicitement.
    //
    // Trouvée le 29/07 : l'ADR-0003 parlait d'une « table `audit_log` », nom
    // qui n'a jamais existé — le journal est réalisé en `audit_chain` et
    // `audit_content`. La première passe ne pouvait pas le voir : elle ne
    // regarde que les jetons `table.colonne`.
    //
    // Le déclencheur est le mot « table » suivi d'un jeton entre accents
    // graves. Étroit exprès : une prose qui parle de « la table des
    // mandats » sans accents graves ne désigne pas un objet du schéma, et
    // n'a pas à être vérifiée.
    let mut unknown_tables = Vec::new();
    for path in &documents {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let lowered = text.to_lowercase();
        let mut from = 0;
        // Déclencheur STRICT : le mot « table » immédiatement suivi de
        // l'accent grave.
        //
        // J'ai essayé de l'élargir — accepter quelques mots entre les deux,
        // pour attraper « la table principale `x` ». L'essai a produit cinq
        // faux positifs d'un coup, dont « toute table portant `org_id` », où
        // le jeton désigne une COLONNE. Une garde qui crie à tort finit
        // désactivée, et l'on aurait échangé une protection réelle contre
        // une fausse. Le déclencheur reste donc étroit.
        //
        // Ce qu'il ne voit pas, et c'est écrit plutôt que masqué : un nom de
        // table cité sans le mot « table » juste avant — « le journal vit
        // dans `audit_log` » — passe. Aucune règle textuelle ne distingue
        // ce cas d'une mention de colonne ou de fonction sans produire ce
        // bruit-là.
        while let Some(at) = lowered[from..].find("table `") {
            let start = from + at + "table `".len();
            from = start;
            let Some(end) = lowered[start..].find('`') else {
                break;
            };
            let name = &lowered[start..start + end];
            if !name.is_empty()
                && name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                && !schema.contains_key(name)
                && !PLANNED_TABLES.contains(&name)
            {
                unknown_tables.push(format!(
                    "{} nomme la table `{name}`, qui n'existe pas",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
        }
    }
    assert!(
        unknown_tables.is_empty(),
        "table(s) nommée(s) dans la documentation et absente(s) du schéma : \
         {unknown_tables:?}. Si la table est PRÉVUE et non construite, \
         l'inscrire dans PLANNED_TABLES avec son jalon — l'y ajouter est un \
         acte conscient, et c'est le but : un lecteur ne peut pas deviner \
         seul si un nom désigne l'existant ou le projeté."
    );
}
