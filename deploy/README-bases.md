# Remonter les images de base épinglées

`deploy/Containerfile` épingle ses bases **par digest**, pas par tag : sans
cela, l'image signée par cosign et décrite par le SBOM ne serait pas
reproductible d'une semaine à l'autre.

**La contrepartie est réelle et doit être tenue** : un digest épinglé ne
reçoit plus les correctifs de sécurité de sa base. Un digest jamais remonté
est PIRE qu'un tag mobile — il fige une base vulnérable tout en donnant
l'apparence de la rigueur.

## Quand

À chaque session de maintenance, et au minimum une fois par mois. Le SBOM
publié par la CI liste ce que contient réellement l'image : c'est lui qui
dit si une base traîne.

## Comment (aucun outil de conteneur requis)

```powershell
function Get-Digest($repo, $tag) {
  $t = Invoke-RestMethod "https://auth.docker.io/token?service=registry.docker.io&scope=repository:$repo`:pull"
  $h = @{ Authorization = "Bearer $($t.token)"
          Accept = "application/vnd.oci.image.index.v1+json,application/vnd.docker.distribution.manifest.list.v2+json" }
  (Invoke-WebRequest "https://registry-1.docker.io/v2/$repo/manifests/$tag" -Headers $h -Method Head -UseBasicParsing).Headers["Docker-Content-Digest"]
}
Get-Digest 'library/rust'   '1-bookworm'
Get-Digest 'library/debian' 'bookworm-slim'
```

Reporter les digests obtenus dans `Containerfile`, committer avec le tag
correspondant en message, et laisser la CI reconstruire et re-signer.

## État au 29/07/2026

| Base | Tag | Digest épinglé |
|---|---|---|
| `library/rust` | `1-bookworm` | `sha256:77fac8b9…dc3fa` |
| `library/debian` | `bookworm-slim` | `sha256:7b140f37…75818` |
