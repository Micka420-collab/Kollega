# NON VÉRIFIÉ localement — Python absent de la machine de développement.
# Cette implémentation de référence est écrite DEPUIS LA SPÉCIFICATION
# (docs/encodage-canonique.md), délibérément sans regarder le code Rust,
# pour qu'un test différentiel prouve quelque chose : si elle était
# traduite du Rust, elle en reproduirait les erreurs.
#
# Exécutée par la CI (étape différentielle ajoutée le 28/07/2026) :
# le test Rust `diff_vectors.rs` génère >= 12 000 vecteurs d'encodage et
# 2 000 empreintes complètes ; ce script les rejoue et la CI compare
# octet à octet. Modes :
#   python3 canonical.py           < vectors.jsonl  -> encodages, un par ligne
#   python3 canonical.py --hashes  < hashes.jsonl   -> empreintes hex
#
# Toute divergence Rust/Python est d'abord un défaut de SPÉCIFICATION à
# documenter, pas un bug à corriger en douce (tools/reference/README.md).
#
# Correction du 28/07/2026 (défaut de premier passage, trouvé par relecture
# avant toute exécution) : le bloc __main__ précédait la définition de
# _from_json — NameError garanti en mode script. Bloc déplacé en fin de
# fichier ; les fonctions d'encodage et de hachage sont inchangées.

from __future__ import annotations

import hashlib
import json
import sys
from typing import Any


def encode(value: Any) -> str:
    """Encode une valeur canonique selon docs/encodage-canonique.md (v3).

    Valeurs admises (portées par des tuples typés pour lever l'ambiguïté
    de Python entre bool/int et pour distinguer Null) :
      ("null",)            -> null
      ("bool", b)          -> true / false
      ("int", i)           -> décimal signé (i doit tenir sur un i64)
      ("text", s)          -> "..." échappé
      ("array", [v, ...])  -> [..]
      ("object", {k: v})   -> {"k":v,..} clés triées par octets UTF-8
    """
    tag = value[0]
    if tag == "null":
        return "null"
    if tag == "bool":
        return "true" if value[1] else "false"
    if tag == "int":
        i = value[1]
        if not (-(2**63) <= i < 2**63):
            raise ValueError("entier hors i64")
        return str(i)
    if tag == "text":
        return _encode_text(value[1])
    if tag == "array":
        return "[" + ",".join(encode(v) for v in value[1]) + "]"
    if tag == "object":
        items = sorted(value[1].items(), key=lambda kv: kv[0].encode("utf-8"))
        return "{" + ",".join(_encode_text(k) + ":" + encode(v) for k, v in items) + "}"
    raise ValueError(f"tag inconnu : {tag!r}")


def _encode_text(s: str) -> str:
    out = ['"']
    for ch in s:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        elif ord(ch) < 0x20:
            out.append("\\u%04x" % ord(ch))
        else:
            out.append(ch)
    out.append('"')
    return "".join(out)


def canonical_record(action: str, actor: str, height: int, org_id: str, payload: Any) -> str:
    """Enregistrement d'audit — ordre des champs figé (spec §3)."""
    return (
        "{"
        + '"action":' + _encode_text(action)
        + ',"actor":' + _encode_text(actor)
        + ',"height":' + str(height)
        + ',"org_id":' + _encode_text(org_id)
        + ',"payload":' + encode(payload)
        + "}"
    )


def entry_hash(
    prev_hash: bytes | None,
    action: str,
    actor: str,
    height: int,
    org_id: str,
    payload: Any,
    timestamp_micros: int,
) -> str:
    """SHA-256(prefixe_prev(32o) || enregistrement || horodatage) — spec §4."""
    h = hashlib.sha256()
    h.update(prev_hash if prev_hash is not None else bytes(32))
    h.update(canonical_record(action, actor, height, org_id, payload).encode("utf-8"))
    h.update(str(timestamp_micros).encode("utf-8"))
    return h.hexdigest()


def _from_json(node: Any) -> Any:  # pragma: no cover - utilitaire de test
    """Reconstruit une valeur taguée depuis sa forme JSON de transport."""
    tag = node[0]
    if tag in ("null", "bool", "int", "text"):
        return tuple(node)
    if tag == "array":
        return ("array", [_from_json(v) for v in node[1]])
    if tag == "object":
        return ("object", {k: _from_json(v) for k, v in node[1].items()})
    raise ValueError(f"tag inconnu : {tag!r}")


def _run_encode() -> None:
    """Mode différentiel encodage : {"value": <valeur taguée>} par ligne."""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        spec = json.loads(line)
        print(encode(_from_json(spec["value"])))


def _run_hashes() -> None:
    """Mode différentiel empreintes : un enregistrement complet par ligne."""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        spec = json.loads(line)
        prev = bytes.fromhex(spec["prev"]) if spec["prev"] is not None else None
        print(
            entry_hash(
                prev,
                spec["action"],
                spec["actor"],
                spec["height"],
                spec["org_id"],
                _from_json(spec["payload"]),
                spec["ts"],
            )
        )


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--hashes":
        _run_hashes()
    else:
        _run_encode()
