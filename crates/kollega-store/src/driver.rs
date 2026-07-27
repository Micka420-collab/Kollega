//! Pilote de la tranche verticale : la machine à états rencontre PostgreSQL.
//!
//! C'est ici que les coutures se cousent, AU-DESSUS du point de passage
//! unique ([`crate::Db::org_transaction`]) :
//! - l'état de tâche vit en base dans son ENVELOPPE versionnée, relu et
//!   réécrit à chaque pas — rien en mémoire entre deux pas ;
//! - le solde d'organisation est LU ET VERROUILLÉ (`FOR UPDATE`) dans la
//!   transaction du pas, jamais repris de l'instantané sérialisé
//!   ([`kollega_runtime::Budget::refreshed`], règle de
//!   `docs/credits-concurrence.md`) ;
//! - chaque événement d'audit de la machine devient UNE attestation dans la
//!   chaîne (`audit_chain` : hauteur, lien, empreinte — ajout seul, porté
//!   par les GRANT) et UN contenu adressé par `(org_id, digest)` dans
//!   `audit_content` (purgeable, invariant 12) ; la chaîne n'atteste que
//!   des empreintes : purger le contenu ne la casse pas ;
//! - une FOURCHE de chaîne (deux écrivains, même hauteur) est rendue
//!   impossible par `PRIMARY KEY (org_id, height)` : le perdant voit une
//!   violation d'unicité et LE PAS ENTIER est rejoué (l'état n'ayant pas
//!   été committé, la reprise est saine) ;
//! - l'horodatage est produit ICI (la machine n'a pas d'horloge) en
//!   microsecondes Unix, stocké en BIGINT : l'aller-retour ne peut pas
//!   tronquer ce qui a été haché.
//!
//! LIMITE CONNUE, consignée : le trait `PolicyEngine` de la machine ne
//! transporte que le NOM de l'outil — les bornes riches de `kollega-policy`
//! (montant, destinataires, chemins) ne sont pas encore atteignables depuis
//! la boucle. L'adaptateur [`RulesPolicy`] délègue au vrai moteur, mais sur
//! une requête réduite au nom.

use kollega_audit::chain::{ChainedEntry, EntryContent, Hash32, OrgChain};
use kollega_audit::CanonicalValue;
use kollega_core::{Cents, Decision, OrgId, TaskStatus};
use kollega_policy::{decide, ToolCallRequest, ToolRule};
use kollega_runtime::machine::{
    drive, ApprovalDecision, AuditEvent, ModelProvider, PolicyEngine, TaskState, TaskStateEnvelope,
    ToolRunner,
};
use sha2::{Digest as _, Sha256};
use sqlx::{Postgres, Row as _, Transaction};
use uuid::Uuid;

use crate::{Db, StoreError};

/// Nombre de rejeux d'un pas quand un autre écrivain a pris la hauteur de
/// chaîne visée (violation d'unicité sur `(org_id, height)`).
const CHAIN_RETRIES: u32 = 3;

/// Résultat d'un pas de tâche persisté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceStep {
    /// Statut de la tâche après le pas.
    pub status: TaskStatus,
    /// Conclusion, si la tâche a abouti.
    pub conclusion: Option<String>,
    /// Solde d'organisation APRÈS le pas (celui écrit en base).
    pub org_balance: Cents,
}

/// Adaptateur : la machine délègue au moteur de politiques réel.
struct RulesPolicy<'a> {
    rules: &'a [ToolRule],
}

impl PolicyEngine for RulesPolicy<'_> {
    fn decide(&self, tool: &str) -> Decision {
        decide(
            self.rules,
            &ToolCallRequest {
                tool_name: tool.to_owned(),
                ..ToolCallRequest::default()
            },
        )
        .decision
    }
}

/// Microsecondes Unix de l'horloge de la périphérie — la machine reste sans
/// horloge. Une horloge antérieure à 1970 donne 0 plutôt qu'une panique.
fn now_micros() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Nom d'action d'un événement de machine — stable, c'est ce qui est haché.
fn event_action(event: &AuditEvent) -> &'static str {
    match event {
        AuditEvent::TaskStarted => "task_started",
        AuditEvent::ToolCallIntended { .. } => "tool_call_intended",
        AuditEvent::ToolCallCompleted { .. } => "tool_call_completed",
        AuditEvent::ToolCallDenied { .. } => "tool_call_denied",
        AuditEvent::ApprovalRequested { .. } => "approval_requested",
        AuditEvent::ApprovalResolved { .. } => "approval_resolved",
        AuditEvent::TaskFinished { .. } => "task_finished",
    }
}

/// Charge utile d'ATTESTATION : l'empreinte du contenu, rien d'autre.
///
/// UNE SEULE fonction, partagée entre l'écriture et la vérification : les
/// octets hachés à l'append et les octets recalculés au verify sont produits
/// par le même code — l'écart est inexprimable.
fn attestation_payload(digest_hex: &str) -> CanonicalValue {
    let mut map = std::collections::BTreeMap::new();
    map.insert(
        "content_digest".to_owned(),
        CanonicalValue::Text(digest_hex.to_owned()),
    );
    CanonicalValue::Object(map)
}

/// Forme sérialisée d'un statut de tâche (verrouillée par les tests de
/// `kollega-core`) — c'est la colonne `tasks.status`.
fn status_str(status: TaskStatus) -> Result<String, StoreError> {
    match serde_json::to_value(status) {
        Ok(serde_json::Value::String(s)) => Ok(s),
        other => Err(StoreError::CorruptState(format!(
            "statut non textuel : {other:?}"
        ))),
    }
}

/// Ajoute une attestation à la chaîne de l'organisation et son contenu au
/// dépôt de contenu, dans la transaction courante.
///
/// Une violation d'unicité sur `(org_id, height)` — un autre écrivain a pris
/// la hauteur — ressort en [`StoreError::ChainConflict`] : l'appelant rejoue
/// le pas entier.
async fn append_attestation(
    tx: &mut Transaction<'static, Postgres>,
    org_id: Uuid,
    actor: &str,
    action: &str,
    content_json: &str,
) -> Result<(), StoreError> {
    let digest: [u8; 32] = Sha256::digest(content_json.as_bytes()).into();
    let digest_hex = hex::encode(digest);

    // Queue de chaîne — la RLS restreint déjà à l'organisation du contexte.
    let tail =
        sqlx::query("SELECT height, entry_hash FROM audit_chain ORDER BY height DESC LIMIT 1")
            .fetch_optional(&mut **tx)
            .await?;
    let (height, prev_hash): (i64, Option<Hash32>) = match tail {
        None => (0, None),
        Some(row) => {
            let prev_height: i64 = row.get(0);
            let bytes: Vec<u8> = row.get(1);
            let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
                StoreError::CorruptState(
                    "empreinte de chaîne d'une autre taille que 32 octets".into(),
                )
            })?;
            (prev_height + 1, Some(Hash32(bytes)))
        }
    };

    let content = EntryContent {
        actor: actor.to_owned(),
        action: action.to_owned(),
        payload: attestation_payload(&digest_hex),
        timestamp_micros: now_micros(),
    };
    let entry_hash = OrgChain::new(OrgId::new(org_id)).entry_hash(
        u64::try_from(height).unwrap_or(0),
        prev_hash.as_ref(),
        &content,
    );

    let inserted = sqlx::query(
        "INSERT INTO audit_chain \
         (org_id, height, prev_hash, entry_hash, actor, action, content_digest, timestamp_micros) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(org_id)
    .bind(height)
    .bind(prev_hash.as_ref().map(|h| h.0.to_vec()))
    .bind(entry_hash.0.to_vec())
    .bind(&content.actor)
    .bind(&content.action)
    .bind(digest.to_vec())
    .bind(content.timestamp_micros)
    .execute(&mut **tx)
    .await;
    match inserted {
        Ok(_) => {}
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
            return Err(StoreError::ChainConflict);
        }
        Err(e) => return Err(e.into()),
    }

    // Contenu adressé par (org_id, digest) : même contenu = même ligne.
    sqlx::query(
        "INSERT INTO audit_content (org_id, digest, content) VALUES ($1, $2, $3) \
         ON CONFLICT (org_id, digest) DO NOTHING",
    )
    .bind(org_id)
    .bind(digest.to_vec())
    .bind(content_json)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Crée une tâche persistée : enveloppe scellée, statut `pending`.
///
/// Le plafond vient de l'appelant ; le solde initial du budget est lu en
/// base au moment du pas (jamais figé ici au-delà de l'amorçage).
pub async fn create_task(
    db: &Db,
    org_id: Uuid,
    task_id: Uuid,
    ceiling: Cents,
    max_iterations: u32,
) -> Result<(), StoreError> {
    let mut tx = db.org_transaction(org_id).await?;
    let balance: i64 =
        sqlx::query("SELECT balance_cents FROM credits WHERE org_id = $1 FOR UPDATE")
            .bind(org_id)
            .fetch_one(&mut *tx)
            .await?
            .get(0);
    let budget = kollega_runtime::Budget::new(ceiling, Cents(balance))
        .map_err(|e| StoreError::Accounting(e.to_string()))?;
    let state = TaskState::new(max_iterations, budget);
    let status = status_str(state.status)?;
    let envelope = serde_json::to_string(&TaskStateEnvelope::seal(state))
        .map_err(|e| StoreError::CorruptState(e.to_string()))?;
    sqlx::query("INSERT INTO tasks (id, org_id, state, status) VALUES ($1, $2, $3::jsonb, $4)")
        .bind(task_id)
        .bind(org_id)
        .bind(envelope)
        .bind(status)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Fait avancer une tâche d'un PAS persisté : relecture de l'enveloppe,
/// solde verrouillé et rafraîchi, machine, attestations, écritures, commit.
///
/// Rejoué en entier (état non committé) si un autre écrivain a pris la
/// hauteur de chaîne visée. ATTENTION, consigné : avec un vrai modèle, ce
/// rejeu ré-appellerait le modèle — l'idempotence du rejeu est une dette
/// documentée de `docs/credits-concurrence.md`, elle se règle au moment du
/// vrai `ModelProvider`.
pub async fn run_task_step(
    db: &Db,
    org_id: Uuid,
    task_id: Uuid,
    model: &dyn ModelProvider,
    tools: &dyn ToolRunner,
    rules: &[ToolRule],
    approval: Option<ApprovalDecision>,
) -> Result<SliceStep, StoreError> {
    let mut last = StoreError::ChainConflict;
    for _ in 0..CHAIN_RETRIES {
        match try_task_step(db, org_id, task_id, model, tools, rules, approval).await {
            Err(StoreError::ChainConflict) => last = StoreError::ChainConflict,
            other => return other,
        }
    }
    Err(last)
}

async fn try_task_step(
    db: &Db,
    org_id: Uuid,
    task_id: Uuid,
    model: &dyn ModelProvider,
    tools: &dyn ToolRunner,
    rules: &[ToolRule],
    approval: Option<ApprovalDecision>,
) -> Result<SliceStep, StoreError> {
    let mut tx = db.org_transaction(org_id).await?;

    // L'état, verrouillé : un seul pas à la fois par tâche.
    let row = sqlx::query("SELECT state::text FROM tasks WHERE id = $1 FOR UPDATE")
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(StoreError::TaskNotFound)?;
    let envelope_json: String = row.get(0);
    let envelope: TaskStateEnvelope = serde_json::from_str(&envelope_json)
        .map_err(|e| StoreError::CorruptState(e.to_string()))?;
    let mut state = envelope.into_state();

    // Le solde RÉEL, verrouillé — jamais l'instantané sérialisé.
    let balance: i64 =
        sqlx::query("SELECT balance_cents FROM credits WHERE org_id = $1 FOR UPDATE")
            .bind(org_id)
            .fetch_one(&mut *tx)
            .await?
            .get(0);
    let balance_before = balance;
    state.budget = state
        .budget
        .refreshed(Cents(balance))
        .map_err(|e| StoreError::Accounting(e.to_string()))?;

    // La machine — pure, sans horloge, sans base.
    let audit_before = state.audit.len();
    let policy = RulesPolicy { rules };
    drive(&mut state, model, &policy, tools, approval);

    // Chaque événement nouveau : une attestation + un contenu.
    let actor = task_id.to_string();
    for event in &state.audit[audit_before..] {
        let content_json =
            serde_json::to_string(event).map_err(|e| StoreError::CorruptState(e.to_string()))?;
        append_attestation(&mut tx, org_id, &actor, event_action(event), &content_json).await?;
    }

    // Écritures : l'état (enveloppe scellée), le statut, le solde décidé
    // par le noyau comptable — le SQL persiste, il ne décide pas.
    let status = status_str(state.status)?;
    let envelope = serde_json::to_string(&TaskStateEnvelope::seal(state.clone()))
        .map_err(|e| StoreError::CorruptState(e.to_string()))?;
    sqlx::query(
        "UPDATE tasks SET state = $2::jsonb, status = $3, updated_at = now() WHERE id = $1",
    )
    .bind(task_id)
    .bind(envelope)
    .bind(&status)
    .execute(&mut *tx)
    .await?;
    let new_balance = state.budget.org_balance();
    debug_assert!(new_balance.0 <= balance_before);
    sqlx::query("UPDATE credits SET balance_cents = $2 WHERE org_id = $1")
        .bind(org_id)
        .bind(new_balance.0)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(SliceStep {
        status: state.status,
        conclusion: state.conclusion.clone(),
        org_balance: new_balance,
    })
}

/// Rapport de vérification de la chaîne d'une organisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainCheck {
    /// Nombre d'entrées vérifiées.
    pub entries: u64,
}

/// Relit la chaîne d'audit de l'organisation et la vérifie de bout en bout.
///
/// La charge utile de chaque attestation est RECONSTRUITE depuis les
/// colonnes par la même fonction qui l'a produite à l'écriture : si les
/// octets diffèrent, la vérification casse — c'est le but.
pub async fn verify_org_chain(db: &Db, org_id: Uuid) -> Result<ChainCheck, StoreError> {
    let mut tx = db.org_transaction(org_id).await?;
    let rows = sqlx::query(
        "SELECT height, prev_hash, entry_hash, actor, action, content_digest, timestamp_micros \
         FROM audit_chain ORDER BY height",
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let height: i64 = row.get(0);
        let prev: Option<Vec<u8>> = row.get(1);
        let hash: Vec<u8> = row.get(2);
        let digest: Option<Vec<u8>> = row.get(5);
        let to32 = |v: Vec<u8>| -> Result<[u8; 32], StoreError> {
            v.try_into().map_err(|_| {
                StoreError::CorruptState("empreinte d'une autre taille que 32 octets".into())
            })
        };
        let digest_hex = match digest {
            Some(d) => hex::encode(to32(d)?),
            None => String::new(),
        };
        entries.push(ChainedEntry {
            content: EntryContent {
                actor: row.get(3),
                action: row.get(4),
                payload: attestation_payload(&digest_hex),
                timestamp_micros: row.get(6),
            },
            height: u64::try_from(height)
                .map_err(|_| StoreError::CorruptState("hauteur négative".into()))?,
            prev_hash: prev.map(to32).transpose()?.map(Hash32),
            hash: Hash32(to32(hash)?),
        });
    }
    OrgChain::new(OrgId::new(org_id))
        .verify(&entries)
        .map_err(|brk| StoreError::ChainBroken(format!("{brk:?}")))?;
    Ok(ChainCheck {
        entries: entries.len() as u64,
    })
}

/// Purge RGPD du CONTENU d'audit d'une organisation (invariant 12) —
/// la chaîne d'attestations, elle, reste intacte et vérifiable.
///
/// La purge est TRACÉE : une attestation `content_purged` est ajoutée à la
/// chaîne, portant le nombre de lignes purgées.
pub async fn purge_org_content(db: &Db, org_id: Uuid) -> Result<u64, StoreError> {
    let mut tx = db.org_transaction(org_id).await?;
    let purged = sqlx::query("DELETE FROM audit_content")
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let content_json = format!("{{\"purged_rows\":{purged}}}");
    append_attestation(&mut tx, org_id, "system", "content_purged", &content_json).await?;
    tx.commit().await?;
    Ok(purged)
}
