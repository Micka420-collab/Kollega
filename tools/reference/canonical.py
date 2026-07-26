# NON VÉRIFIÉ — Python absent de la machine de développement (28/07/2026).
# Cette implémentation de référence n'a JAMAIS été exécutée ici. Elle est
# écrite DEPUIS LA SPÉCIFICATION (docs/encodage-canonique.md), délibérément
# sans regarder le code Rust, pour qu'un test différentiel prouve quelque
# chose : si elle était traduite du Rust, elle en reproduirait les erreurs.
#
# Rôle : encodeur canonique de référence et générateur de vecteurs, pour un
# test différentiel Rust <-> Python (>= 10 000 vecteurs) à exécuter dès qu'un
# interpréteur Python est disponible. Voir tools/reference/README.md.
#
# Toute divergence Rust/Python est d'abord un défaut de SPÉCIFICATION à
# documenter, pas un bug à corriger en douce.

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


if __name__ == "__main__":
    # Mode différentiel : lit des vecteurs JSON sur stdin (un par ligne :
    # {"value": <valeur taguée>}) et écrit l'encodage sur stdout, pour
    # comparaison octet à octet avec la sortie Rust.
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        spec = json.loads(line)
        print(encode(_from_json(spec["value"])))


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
