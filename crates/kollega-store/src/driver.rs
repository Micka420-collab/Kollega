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

use kollega_audit::chain::{ChainTip, EntryContent, Hash32, OrgChain, StoredEntry};
use kollega_audit::content::{AuditContent, ContentDigest, ContentPayload};
use kollega_audit::records::{verify_sequence, AbandonReason, AuditRecord, SequenceReport};
use kollega_audit::repository::{AuditChainRepository, AuditContentRepository};
use kollega_audit::CanonicalValue;
use kollega_core::{Cents, Decision, OrgId, TaskStatus, ToolCallId};
use kollega_policy::{decide, ToolCallRequest, ToolRule};
use kollega_runtime::machine::{
    drive, ApprovalDecision, AuditEvent, ModelProvider, PolicyEngine, TaskState, TaskStateEnvelope,
    ToolRunner,
};
use sha2::{Digest as _, Sha256};
use sqlx::{Postgres, Row as _, Transaction};
use std::cell::RefCell;
use std::collections::BTreeMap;
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

/// Identité DÉRIVÉE d'un appel d'outil : `SHA-256(task_id || iteration)`.
///
/// Dérivable, donc STABLE : deux exécutions du même pas calculent la même
/// identité, et la seconde reconnaît l'effet de la première. Un identifiant
/// tiré au hasard à chaque tentative rendrait l'idempotence impossible.
/// Les bits de version/variante sont posés pour que la valeur soit un UUID
/// bien formé (v8, « custom »).
fn derive_tool_call_id(task_id: Uuid, iteration: u32) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(task_id.as_bytes());
    hasher.update(iteration.to_be_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0F) | 0x80; // version 8
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variante RFC 4122
    Uuid::from_bytes(bytes)
}

/// Exécuteur IDEMPOTENT : n'accomplit un effet qu'une seule fois.
///
/// Les effets déjà réalisés pour cette tâche sont chargés AVANT le pas
/// (`known`) ; un appel dont l'effet est connu rend le résultat enregistré
/// SANS toucher à l'exécuteur réel. Les effets nouveaux sont collectés
/// (`performed`) et persistés dans la transaction du pas : soit le pas
/// entier est validé — effet et sa trace ensemble —, soit rien.
struct IdempotentTools<'a> {
    inner: &'a dyn ToolRunner,
    known: BTreeMap<u32, String>,
    performed: RefCell<Vec<(u32, String, String)>>,
}

impl ToolRunner for IdempotentTools<'_> {
    fn run(&self, tool: &str, iteration: u32) -> String {
        if let Some(recorded) = self.known.get(&iteration) {
            // L'effet a DÉJÀ eu lieu dans le monde réel : on rend son
            // résultat, on ne le refait pas. C'est ici que le second mail
            // n'est pas envoyé.
            return recorded.clone();
        }
        let result = self.inner.run(tool, iteration);
        self.performed
            .borrow_mut()
            .push((iteration, tool.to_owned(), result.clone()));
        result
    }
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

/// Horodatage de l'horloge de la périphérie — la machine reste sans
/// horloge. Toute construction passe par [`kollega_core::Timestamp`]
/// (bloc 3b) : la précision est la microseconde PAR LE TYPE, l'écart entre
/// ce qui est haché et ce qui fait l'aller-retour est inexprimable.
fn now_micros() -> i64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i128::try_from(d.as_nanos()).unwrap_or(i128::MAX))
        .unwrap_or(0);
    kollega_core::Timestamp::from_unix_nanos(nanos).as_micros()
}

/// Appel d'outil concerné par un événement, s'il y en a un.
///
/// C'est le PONT entre la machine (qui identifie ses appels par
/// `(task_id, iteration)`) et les [`AuditRecord`] du journal chaîné : sans
/// lui, le validateur de séquence n'aurait jamais de vraies données à
/// examiner.
fn event_tool_call(task_id: Uuid, event: &AuditEvent) -> Option<ToolCallId> {
    let iteration = match event {
        AuditEvent::ToolCallIntended { iteration, .. }
        | AuditEvent::ToolCallCompleted { iteration, .. }
        | AuditEvent::ToolCallDenied { iteration, .. } => *iteration,
        // Une demande de validation n'ouvre ni ne clôt un appel : l'appel
        // est déjà ouvert par son intention, et il se clôra à l'exécution
        // ou au refus. L'attacher ici en ferait un doublon d'ouverture.
        AuditEvent::ApprovalRequested { .. }
        | AuditEvent::ApprovalResolved { .. }
        | AuditEvent::TaskStarted
        | AuditEvent::TaskFinished { .. } => return None,
    };
    Some(ToolCallId::new(derive_tool_call_id(task_id, iteration)))
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

/// Dépôt de CHAÎNE adossé à PostgreSQL, dans la transaction du pas.
///
/// Bloc 3f : le pilote ne parle plus à `audit_chain` en SQL libre, il passe
/// par ce dépôt — dont le trait n'a que `append` et `read`. Une suppression
/// dans la chaîne n'est donc pas seulement interdite par les GRANT, elle
/// n'est pas EXPRIMABLE dans la surface que le pilote utilise.
pub struct PgAuditChain<'t> {
    tx: &'t mut Transaction<'static, Postgres>,
    org_id: Uuid,
}

impl AuditChainRepository for PgAuditChain<'_> {
    type Error = StoreError;

    async fn append(
        &mut self,
        actor: &str,
        action: &str,
        tool_call: Option<ToolCallId>,
        content: &AuditContent,
    ) -> Result<(), StoreError> {
        let digest = content.digest();
        // Queue de chaîne — la RLS restreint déjà à l'organisation courante.
        let tail =
            sqlx::query("SELECT height, entry_hash FROM audit_chain ORDER BY height DESC LIMIT 1")
                .fetch_optional(&mut **self.tx)
                .await?;
        let tip = match tail {
            None => None,
            Some(row) => {
                let prev_height: i64 = row.get(0);
                let bytes: Vec<u8> = row.get(1);
                let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
                    StoreError::CorruptState(
                        "empreinte de chaîne d'une autre taille que 32 octets".into(),
                    )
                })?;
                Some(ChainTip {
                    height: u64::try_from(prev_height).map_err(|_| {
                        StoreError::CorruptState("hauteur de chaîne négative".into())
                    })?,
                    hash: Hash32(bytes),
                })
            }
        };

        // L'entrée est PRODUITE par le domaine : son empreinte est calculée,
        // elle ne peut pas mentir (bloc 3c). On persiste ce qu'il a produit.
        let entry = OrgChain::new(OrgId::new(self.org_id)).append(
            tip,
            EntryContent {
                actor: actor.to_owned(),
                action: action.to_owned(),
                payload: attestation_payload(&digest.to_hex()),
                timestamp_micros: now_micros(),
            },
        );

        // ON CONFLICT CIBLÉ, et le ciblage est tout le sujet : en
        // PostgreSQL, une violation de contrainte AVORTE la transaction
        // entière (25P02) — l'attraper « au passage » pour continuer ne
        // marche pas, tout ce qui suit échoue. Il faut donc dire à l'avance
        // quel conflit est acceptable.
        //   * attestation déjà présente (pas rejoué) → DO NOTHING, la
        //     transaction reste saine et l'écriture devient idempotente ;
        //   * hauteur déjà prise (course d'écrivains) → NON couvert ici,
        //     l'erreur remonte et le pas entier se rejoue. C'est voulu.
        let inserted = sqlx::query(
            "INSERT INTO audit_chain \
             (org_id, height, prev_hash, entry_hash, actor, action, tool_call_id, \
              content_digest, timestamp_micros) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT ON CONSTRAINT audit_chain_one_attestation_per_call_action \
             DO NOTHING",
        )
        .bind(self.org_id)
        .bind(i64::try_from(entry.height()).unwrap_or(i64::MAX))
        .bind(entry.prev_hash().map(|h| h.0.to_vec()))
        .bind(entry.hash().0.to_vec())
        .bind(&entry.content().actor)
        .bind(&entry.content().action)
        .bind(tool_call.map(|id| *id.as_uuid()))
        .bind(digest.as_bytes().to_vec())
        .bind(entry.content().timestamp_micros)
        .execute(&mut **self.tx)
        .await;
        match inserted {
            Ok(_) => Ok(()),
            // Ne reste que le conflit de HAUTEUR : un autre écrivain a
            // avancé la chaîne, le pas entier se rejoue.
            Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
                Err(StoreError::ChainConflict)
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn read(&mut self) -> Result<Vec<StoredEntry>, StoreError> {
        let rows = sqlx::query(
            "SELECT height, prev_hash, entry_hash, actor, action, content_digest, timestamp_micros \
             FROM audit_chain ORDER BY height",
        )
        .fetch_all(&mut **self.tx)
        .await?;
        let to32 = |v: Vec<u8>| -> Result<[u8; 32], StoreError> {
            v.try_into().map_err(|_| {
                StoreError::CorruptState("empreinte d'une autre taille que 32 octets".into())
            })
        };
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let height: i64 = row.get(0);
            let prev: Option<Vec<u8>> = row.get(1);
            let hash: Vec<u8> = row.get(2);
            let digest: Option<Vec<u8>> = row.get(5);
            let digest_hex = match digest {
                Some(d) => hex::encode(to32(d)?),
                None => String::new(),
            };
            // StoredEntry : ce qui sort du stockage est une PRÉTENTION, pas
            // une preuve — c'est `verify` qui tranche.
            entries.push(StoredEntry {
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
        Ok(entries)
    }
}

/// Dépôt de CONTENU adossé à PostgreSQL — purgeable, lui (invariant 12).
pub struct PgAuditContent<'t> {
    tx: &'t mut Transaction<'static, Postgres>,
    org_id: Uuid,
}

impl AuditContentRepository for PgAuditContent<'_> {
    type Error = StoreError;

    async fn put(&mut self, content: &AuditContent) -> Result<(), StoreError> {
        // Adressé par (org_id, digest) : même contenu = même ligne.
        sqlx::query(
            "INSERT INTO audit_content (org_id, digest, content) VALUES ($1, $2, $3) \
             ON CONFLICT (org_id, digest) DO NOTHING",
        )
        .bind(self.org_id)
        .bind(content.digest().as_bytes().to_vec())
        .bind(content.payload().as_str())
        .execute(&mut **self.tx)
        .await?;
        Ok(())
    }

    async fn read(&mut self, digest: ContentDigest) -> Result<Option<ContentPayload>, StoreError> {
        let row = sqlx::query("SELECT content FROM audit_content WHERE digest = $1")
            .bind(digest.as_bytes().to_vec())
            .fetch_optional(&mut **self.tx)
            .await?;
        Ok(row.map(|r| ContentPayload::new(r.get(0))))
    }

    async fn purge_org(&mut self) -> Result<u64, StoreError> {
        Ok(sqlx::query("DELETE FROM audit_content")
            .execute(&mut **self.tx)
            .await?
            .rows_affected())
    }
}

/// Ajoute une attestation à la chaîne et son contenu au dépôt de contenu,
/// dans la transaction courante — en passant par les DEUX dépôts.
///
/// Une violation d'unicité sur `(org_id, height)` — un autre écrivain a pris
/// la hauteur — ressort en [`StoreError::ChainConflict`] : l'appelant rejoue
/// le pas entier.
async fn append_attestation(
    tx: &mut Transaction<'static, Postgres>,
    org_id: Uuid,
    actor: &str,
    action: &str,
    tool_call: Option<ToolCallId>,
    content_json: &str,
) -> Result<(), StoreError> {
    let content = AuditContent::new(
        OrgId::new(org_id),
        ContentPayload::new(content_json.to_owned()),
    );
    // Le contenu d'abord : une attestation dont le contenu manquerait serait
    // une empreinte sans original. Les emprunts sont séquentiels, jamais
    // chevauchants — un seul dépôt tient la transaction à la fois.
    PgAuditContent { tx, org_id }.put(&content).await?;
    PgAuditChain { tx, org_id }
        .append(actor, action, tool_call, &content)
        .await
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
    for attempt in 0..CHAIN_RETRIES {
        match try_task_step(db, org_id, task_id, model, tools, rules, approval).await {
            Err(StoreError::ChainConflict) => {
                // Le pas perdu a PU exécuter des effets (outil, modèle)
                // avant l'annulation de sa transaction : son effet réel est
                // INCONNU. On l'atteste en ABANDON — jamais en échec, ce
                // serait un mensonge dans la chaîne (bloc 3d) — puis on
                // rejoue le pas.
                attest_step_abandoned(db, org_id, task_id, attempt).await?;
                last = StoreError::ChainConflict;
            }
            other => return other,
        }
    }
    Err(last)
}

/// Atteste l'abandon d'un pas rejoué (effet réel inconnu) — bloc 3d branché
/// sur le chemin de reprise réel : le rejeu après conflit de chaîne.
async fn attest_step_abandoned(
    db: &Db,
    org_id: Uuid,
    task_id: Uuid,
    attempt: u32,
) -> Result<(), StoreError> {
    let content =
        format!("{{\"reason\":\"step_replay_after_chain_conflict\",\"attempt\":{attempt}}}");
    let actor = task_id.to_string();
    // L'attestation d'abandon peut elle-même perdre la course à la hauteur :
    // bornée aux mêmes rejeux que le pas.
    for _ in 0..CHAIN_RETRIES {
        let mut tx = db.org_transaction(org_id).await?;
        match append_attestation(&mut tx, org_id, &actor, "step_abandoned", None, &content).await {
            Ok(()) => {
                tx.commit().await?;
                return Ok(());
            }
            Err(StoreError::ChainConflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(StoreError::ChainConflict)
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

    // Les effets DÉJÀ RÉALISÉS pour cette tâche : ce que le monde a déjà
    // subi et qu'il ne faut surtout pas lui infliger deux fois. Le contenu
    // vit dans audit_content (purgeable) ; une jointure à gauche laisse
    // voir le cas « effet enregistré, contenu purgé », traité en erreur
    // explicite plus bas — jamais en ré-exécution silencieuse.
    let effect_rows = sqlx::query(
        "SELECT e.iteration, c.content \
         FROM tool_call_effects e \
         LEFT JOIN audit_content c ON c.org_id = e.org_id AND c.digest = e.result_digest \
         WHERE e.task_id = $1",
    )
    .bind(task_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut known = BTreeMap::new();
    for row in effect_rows {
        let iteration: i64 = row.get(0);
        let content: Option<String> = row.get(1);
        let iteration = u32::try_from(iteration)
            .map_err(|_| StoreError::CorruptState("itération d'effet hors bornes".into()))?;
        let Some(content) = content else {
            return Err(StoreError::CorruptState(format!(
                "effet de l'itération {iteration} enregistré mais son contenu a été purgé : \
                 rejeu impossible sans risquer de refaire l'effet — tâche à clore à la main"
            )));
        };
        known.insert(iteration, content);
    }

    // La machine — pure, sans horloge, sans base — et son exécuteur rendu
    // idempotent par la mémoire des effets.
    let audit_before = state.audit.len();
    let policy = RulesPolicy { rules };
    let idempotent = IdempotentTools {
        inner: tools,
        known,
        performed: RefCell::new(Vec::new()),
    };
    drive(&mut state, model, &policy, &idempotent, approval);

    // Les effets NOUVEAUX : leur trace part dans la même transaction que le
    // pas. Soit l'effet et sa mémoire sont validés ensemble, soit le pas
    // entier est annulé et l'effet sera reconnu comme non enregistré.
    for (iteration, tool, result) in idempotent.performed.into_inner() {
        // Le résultat passe par le dépôt de contenu, et son empreinte est
        // CALCULÉE par le type du domaine — plus de hachage à la main ici.
        let content = AuditContent::new(OrgId::new(org_id), ContentPayload::new(result));
        let digest = content.digest();
        PgAuditContent {
            tx: &mut tx,
            org_id,
        }
        .put(&content)
        .await?;
        // MÊME PIÈGE, second endroit — révélé par le diagnostic de la run
        // n°36 : ici aussi, attraper le 23505 pour « continuer » aurait
        // avorté la transaction et fait échouer les écritures suivantes
        // (état de tâche, solde). Les DEUX unicités de cette table
        // signifient la même chose — l'effet est déjà enregistré — donc
        // `DO NOTHING` sans cible est correct : la trace existante fait foi.
        sqlx::query(
            "INSERT INTO tool_call_effects \
             (org_id, tool_call_id, task_id, iteration, tool, result_digest) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT DO NOTHING",
        )
        .bind(org_id)
        .bind(derive_tool_call_id(task_id, iteration))
        .bind(task_id)
        .bind(i64::from(iteration))
        .bind(&tool)
        .bind(digest.as_bytes().to_vec())
        .execute(&mut *tx)
        .await?;
    }

    // Chaque événement nouveau : une attestation + un contenu.
    let actor = task_id.to_string();
    for event in &state.audit[audit_before..] {
        let content_json =
            serde_json::to_string(event).map_err(|e| StoreError::CorruptState(e.to_string()))?;
        let tool_call = event_tool_call(task_id, event);
        append_attestation(
            &mut tx,
            org_id,
            &actor,
            event_action(event),
            tool_call,
            &content_json,
        )
        .await?;
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
    let entries = PgAuditChain {
        tx: &mut tx,
        org_id,
    }
    .read()
    .await?;
    OrgChain::new(OrgId::new(org_id))
        .verify(&entries)
        .map_err(|brk| StoreError::ChainBroken(format!("{brk:?}")))?;
    Ok(ChainCheck {
        entries: entries.len() as u64,
    })
}

/// Vérifie la SÉQUENCE des appels d'outils inscrite dans la chaîne réelle.
///
/// Complète [`verify_org_chain`], qui prouve que la chaîne n'a pas été
/// altérée : ici on demande si ce qu'elle raconte a du SENS — pas de
/// clôture sans intention, pas d'intention en double, rien après une
/// clôture. Le rapport distingue les appels OUVERTS (une tâche en cours,
/// une validation en attente, un redémarrage : légitime) des VIOLATIONS
/// (les seules qui invalident) — c'est l'asymétrie du bloc 3e appliquée aux
/// données de production, là où elle ne vivait que dans des tests purs.
pub async fn verify_org_sequence(db: &Db, org_id: Uuid) -> Result<SequenceReport, StoreError> {
    let mut tx = db.org_transaction(org_id).await?;
    let rows = sqlx::query(
        "SELECT action, tool_call_id, content_digest FROM audit_chain \
         WHERE tool_call_id IS NOT NULL ORDER BY height",
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let action: String = row.get(0);
        let tool_call_id: Uuid = row.get(1);
        let digest: Option<Vec<u8>> = row.get(2);
        let digest: [u8; 32] = digest
            .ok_or_else(|| StoreError::CorruptState("attestation sans empreinte".into()))?
            .try_into()
            .map_err(|_| {
                StoreError::CorruptState("empreinte d'une autre taille que 32 octets".into())
            })?;
        let tool_call_id = ToolCallId::new(tool_call_id);
        // La frontière de stockage : l'empreinte est RELUE, pas recalculée
        // — c'est `verify_org_chain` qui garantit qu'elle n'a pas bougé.
        let digest = ContentDigest::from_storage(digest);
        records.push(match action.as_str() {
            "tool_call_intended" => AuditRecord::Intent {
                tool_call_id,
                request: digest,
            },
            "tool_call_completed" => AuditRecord::Outcome {
                tool_call_id,
                result: digest,
            },
            // Un refus CLÔT l'appel : l'outil n'a pas agi, et il n'agira
            // plus. Ce n'est ni un résultat, ni un abandon à effet inconnu.
            "tool_call_denied" => AuditRecord::Abandoned {
                tool_call_id,
                reason: AbandonReason::RestartWithUnknownEffect,
            },
            other => {
                return Err(StoreError::CorruptState(format!(
                    "action {other} porte un identifiant d'appel sans être un événement d'appel"
                )))
            }
        });
    }
    Ok(verify_sequence(&records))
}

/// Purge RGPD du CONTENU d'audit d'une organisation (invariant 12) —
/// la chaîne d'attestations, elle, reste intacte et vérifiable.
///
/// La purge est TRACÉE : une attestation `content_purged` est ajoutée à la
/// chaîne, portant le nombre de lignes purgées.
pub async fn purge_org_content(db: &Db, org_id: Uuid) -> Result<u64, StoreError> {
    let mut tx = db.org_transaction(org_id).await?;
    // `purge_org` est le SEUL retrait exprimable, et il porte son nom : il
    // vit sur le dépôt de CONTENU, jamais sur celui de la chaîne.
    let purged = PgAuditContent {
        tx: &mut tx,
        org_id,
    }
    .purge_org()
    .await?;
    let content_json = format!("{{\"purged_rows\":{purged}}}");
    append_attestation(
        &mut tx,
        org_id,
        "system",
        "content_purged",
        None,
        &content_json,
    )
    .await?;
    tx.commit().await?;
    Ok(purged)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Approfondissement (nuit du 28 au 29/07) — première façon dont
    /// l'idempotence pourrait être FAUSSE sans qu'aucun test ne le voie :
    /// une dérivation instable. Si l'identité changeait d'un appel à
    /// l'autre, un rejeu ne reconnaîtrait jamais l'effet précédent et le
    /// referait — le second mail partirait, en silence.
    #[test]
    fn the_derived_identity_is_stable_across_calls() {
        let task = Uuid::from_u128(0xA11CE);
        assert_eq!(
            derive_tool_call_id(task, 3),
            derive_tool_call_id(task, 3),
            "la dérivation doit être une FONCTION, pas un tirage"
        );
    }

    /// Deuxième façon : une dérivation qui ignorerait la TÂCHE. L'effet
    /// d'une tâche serait alors attribué à une autre — un appel jamais
    /// exécuté passerait pour déjà fait, et le mail ne partirait jamais.
    /// C'est la panne silencieuse la plus coûteuse de tout le dispositif.
    #[test]
    fn two_tasks_at_the_same_iteration_never_share_an_identity() {
        let a = Uuid::from_u128(0xA11CE);
        let b = Uuid::from_u128(0xB0B);
        for iteration in [0u32, 1, 7, u32::MAX] {
            assert_ne!(
                derive_tool_call_id(a, iteration),
                derive_tool_call_id(b, iteration),
                "l'identité doit dépendre de la tâche (itération {iteration})"
            );
        }
    }

    /// Troisième façon : une dérivation qui ignorerait l'ITÉRATION. Le
    /// deuxième appel d'une même tâche serait pris pour le premier — donc
    /// jamais exécuté, et son résultat remplacé par celui du précédent.
    #[test]
    fn two_iterations_of_one_task_never_share_an_identity() {
        let task = Uuid::from_u128(0xA11CE);
        let mut seen = std::collections::BTreeSet::new();
        for iteration in 0..64u32 {
            assert!(
                seen.insert(derive_tool_call_id(task, iteration)),
                "collision d'identité à l'itération {iteration}"
            );
        }
    }

    /// L'identité dérivée reste un UUID BIEN FORMÉ (version 8, variante
    /// RFC 4122) : la colonne est de type `uuid`, et un jour un humain
    /// lira ces valeurs dans un journal.
    #[test]
    fn the_derived_identity_is_a_well_formed_uuid() {
        let id = derive_tool_call_id(Uuid::from_u128(0xA11CE), 0);
        assert_eq!(id.get_version_num(), 8);
        assert_eq!(id.get_variant(), uuid::Variant::RFC4122);
    }
}
