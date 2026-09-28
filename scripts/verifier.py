#!/usr/bin/env python3
"""Vérification de bout en bout, en lecture seule, contre un vrai ENT.

Lance le serveur en mode stdio, appelle chaque outil de consultation et
affiche un résumé (nombre d'éléments, erreurs), sans afficher les contenus.

    export ENT1_IDENTIFIANT=... ENT1_MOT_DE_PASSE=...
    python3 scripts/verifier.py [chemin/vers/ent1-mcp]
"""

import json
import subprocess
import sys

BINAIRE = sys.argv[1] if len(sys.argv) > 1 else "target/release/ent1-mcp"

APPELS = [
    ("profil", {}),
    ("fil_nouveautes", {}),
    ("messagerie_non_lus", {}),
    ("messagerie_lister", {"taille": 5}),
    ("messagerie_lister", {"dossier": "envoyes", "taille": 5}),
    ("messagerie_dossiers", {}),
    ("annuaire_rechercher", {"recherche": ""}),
    ("carnet_lister", {}),
    ("cahier_lister", {}),
    ("actualites_fils", {}),
    ("actualites_lister", {"taille": 3}),
    ("blog_lister", {}),
    ("calendrier_lister", {}),
    ("calendrier_evenements", {}),
    ("documents_lister", {}),
    ("casier_lister", {}),
    ("casier_destinataires", {}),
    ("formulaires_lister", {}),
    ("wiki_lister", {}),
]


def resume(valeur):
    """Décrit la forme d'un résultat sans en révéler le contenu."""
    if isinstance(valeur, dict):
        return ", ".join(
            f"{k}: {len(v)} élément(s)" if isinstance(v, list) else k
            for k, v in valeur.items()
        )
    return type(valeur).__name__


def main():
    proc = subprocess.Popen(
        [BINAIRE],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    )
    suivant = iter(range(1, 10_000))

    def requete(methode, params):
        ident = next(suivant)
        proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": ident, "method": methode, "params": params}) + "\n")
        proc.stdin.flush()
        while True:
            ligne = proc.stdout.readline()
            if not ligne:
                raise SystemExit("le serveur s'est arrêté (voir les journaux ci-dessus)")
            msg = json.loads(ligne)
            if msg.get("id") == ident:
                return msg

    init = requete(
        "initialize",
        {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "verifier", "version": "0"},
        },
    )
    print("serveur :", init["result"]["serverInfo"]["name"], init["result"]["serverInfo"]["version"])
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
    proc.stdin.flush()

    outils = requete("tools/list", {})["result"]["tools"]
    print(f"{len(outils)} outils exposés\n")

    echecs = 0
    for nom, args in APPELS:
        rep = requete("tools/call", {"name": nom, "arguments": args})
        res = rep.get("result", {})
        if "error" in rep or res.get("isError"):
            echecs += 1
            texte = rep.get("error") or res.get("content", [{}])[0].get("text")
            print(f"ÉCHEC {nom} {args}: {texte}")
        else:
            print(f"ok    {nom} {args}: {resume(res.get('structuredContent'))}")

    proc.stdin.close()
    proc.wait(timeout=10)
    print(f"\n{len(APPELS) - echecs}/{len(APPELS)} appels réussis")
    sys.exit(1 if echecs else 0)


if __name__ == "__main__":
    main()
