# ent1-mcp

**Parlez à l'ENT de vos enfants depuis votre assistant IA.**

`ent1-mcp` est un serveur [MCP (Model Context Protocol)](https://modelcontextprotocol.io) pour l'**ENT ONE**, l'espace numérique de travail des écoles construit sur la plateforme **Edifice** (ex-Open Digital Education). Il permet à Claude, ChatGPT ou tout autre assistant compatible MCP de lire et d'écrire dans l'ENT à votre place : messagerie avec les enseignants, carnet de liaison, cahier de texte, actualités de l'école, blogs de classe, calendrier, casier, documents, formulaires…

Écrit en Rust, sans dépendance à un navigateur, il se connecte avec vos identifiants habituels et ne stocke rien.

> [!IMPORTANT]
> Projet communautaire **non officiel**. Il n'est ni développé, ni soutenu, ni validé par Edifice, par l'Éducation nationale ou par une académie. Il utilise l'API interne que l'application web de l'ENT appelle elle-même, qui peut changer sans préavis.

---

## Sommaire

- [Ce que vous pouvez demander](#ce-que-vous-pouvez-demander)
- [Fonctionnalités](#fonctionnalités)
- [Installation](#installation)
- [Configuration](#configuration)
- [Brancher un assistant](#brancher-un-assistant)
- [Sécurité et vie privée](#sécurité-et-vie-privée)
- [Autres ENT Edifice](#autres-ent-edifice)
- [Architecture](#architecture)
- [Développement](#développement)
- [Limites connues](#limites-connues)
- [Feuille de route](#feuille-de-route)
- [Contribuer](#contribuer)
- [Licence](#licence)

---

## Ce que vous pouvez demander

> « Qu'est-ce qui s'est passé sur l'ENT cette semaine ? »

> « Y a-t-il des mots du carnet de liaison que je n'ai pas encore signés ? Signe-les. »

> « Quels sont les devoirs de Léa pour jeudi ? »

> « Écris à la maîtresse de CM1 pour la prévenir que Léa sera absente vendredi matin (rendez-vous chez l'orthophoniste). Montre-moi le message avant de l'envoyer. »

> « Résume-moi les trois dernières actualités de l'école et ajoute les dates importantes à ma liste. »

> « Dépose l'attestation d'assurance dans le casier de l'enseignante. »

L'assistant enchaîne lui-même les outils : il cherche l'enseignante dans l'annuaire, rédige le message, vous le soumet, puis l'envoie.

## Fonctionnalités

47 outils, regroupés par application de l'ENT. Les outils de lecture sont annotés `readOnlyHint`, les suppressions `destructiveHint` : les clients MCP s'en servent pour demander (ou non) votre confirmation.

| Application | Outils | Lecture | Écriture |
|---|---|:-:|:-:|
| **Compte** | `profil`, `deconnexion` | ✅ | |
| **Quoi de neuf ?** | `fil_nouveautes` | ✅ | |
| **Messagerie** | `messagerie_lister`, `messagerie_lire`, `messagerie_non_lus`, `messagerie_envoyer`, `messagerie_enregistrer_brouillon`, `messagerie_joindre_fichier`, `messagerie_telecharger_piece_jointe`, `messagerie_marquer`, `messagerie_deplacer`, `messagerie_supprimer`, `messagerie_restaurer`, `messagerie_rappeler`, `messagerie_dossiers`, `messagerie_creer_dossier` | ✅ | ✅ |
| **Annuaire** | `annuaire_rechercher`, `annuaire_fiche` | ✅ | |
| **Carnet de liaison** | `carnet_lister`, `carnet_signer`, `carnet_repondre` | ✅ | ✅ |
| **Cahier de texte** | `cahier_lister`, `cahier_lire` | ✅ | |
| **Actualités** | `actualites_lister`, `actualites_fils`, `actualites_lire`, `actualites_commenter` | ✅ | ✅ |
| **Blog** | `blog_lister`, `blog_billets`, `blog_commentaires`, `blog_commenter`, `blog_supprimer_commentaire` | ✅ | ✅ |
| **Calendrier** | `calendrier_lister`, `calendrier_evenements` | ✅ | |
| **Espace documentaire** | `documents_lister`, `documents_telecharger`, `documents_deposer` | ✅ | ✅ |
| **Casier** | `casier_lister`, `casier_destinataires`, `casier_deposer`, `casier_telecharger`, `casier_supprimer` | ✅ | ✅ |
| **Formulaires** | `formulaires_lister`, `formulaires_lire` | ✅ | lien |
| **Wiki** | `wiki_lister`, `wiki_lire_page` | ✅ | |

Détails utiles :

- **Texte brut partout.** Le HTML de l'ENT est converti en texte à la lecture ; à l'écriture, votre texte est échappé puis mis en paragraphes. Aucune injection de HTML possible.
- **Réponses structurées.** Chaque outil renvoie un JSON typé (`structuredContent`) avec son schéma de sortie, en plus du texte.
- **Fichiers en base64**, 10 Mo maximum par fichier, dans les deux sens.
- **Dates dans votre fuseau horaire** (`Indian/Reunion` par défaut).
- **Messages rappelables** pendant 60 minutes (`messagerie_rappeler`), comme dans l'ENT.

## Installation

### Avec Cargo

Il faut [Rust](https://rustup.rs) 1.88 ou plus récent.

```bash
cargo install --git https://github.com/edouard-claude/ent1-mcp
```

Le binaire `ent1-mcp` est installé dans `~/.cargo/bin`.

### Depuis les sources

```bash
git clone https://github.com/edouard-claude/ent1-mcp
cd ent1-mcp
cargo build --release
# binaire : target/release/ent1-mcp
```

### Avec Docker (mode HTTP)

```bash
docker build -t ent1-mcp .
docker run --rm -p 8080:8080 \
  -e ENT1_IDENTIFIANT -e ENT1_MOT_DE_PASSE -e ENT1_MCP_JETON \
  ent1-mcp
```

## Configuration

Tout se règle par variables d'environnement. Les **secrets** ne peuvent pas être passés en argument de ligne de commande : ils apparaîtraient dans la liste des processus.

| Variable | Obligatoire | Défaut | Rôle |
|---|:-:|---|---|
| `ENT1_IDENTIFIANT` | ✅ | | Identifiant de connexion à l'ENT (ex. `prenom.nom`) |
| `ENT1_MOT_DE_PASSE` | ✅ | | Mot de passe de l'ENT |
| `ENT1_URL` | | `https://ent1d.ac-reunion.fr` | Adresse de votre ENT |
| `ENT1_FUSEAU` | | `Indian/Reunion` | Fuseau horaire des dates renvoyées |
| `ENT1_TRANSPORT` | | `stdio` | `stdio` (client local) ou `http` (serveur distant) |
| `ENT1_ECOUTE` | | `127.0.0.1:8080` | Adresse d'écoute en mode HTTP |
| `ENT1_MCP_JETON` | en HTTP | | Jeton d'accès du mode HTTP, 32 caractères minimum |
| `ENT1_HOTES` | | boucle locale | Noms d'hôte publics acceptés en HTTP, séparés par des virgules |
| `RUST_LOG` | | `ent1_mcp=info` | Niveau des journaux (écrits sur la sortie d'erreur) |

Un modèle est fourni dans [`.env.exemple`](.env.exemple).

Au démarrage, le serveur ouvre une session sur l'ENT : des identifiants erronés sont signalés tout de suite, pas au premier appel d'outil.

## Brancher un assistant

### Claude Desktop

Ouvrez *Réglages → Développeur → Modifier la configuration* et ajoutez :

```json
{
  "mcpServers": {
    "ent": {
      "command": "/Users/vous/.cargo/bin/ent1-mcp",
      "env": {
        "ENT1_IDENTIFIANT": "prenom.nom",
        "ENT1_MOT_DE_PASSE": "votre-mot-de-passe"
      }
    }
  }
}
```

Redémarrez Claude Desktop : les outils de l'ENT apparaissent dans le menu des connecteurs.

### Claude Code

```bash
claude mcp add ent --env ENT1_IDENTIFIANT=prenom.nom --env ENT1_MOT_DE_PASSE=votre-mot-de-passe -- ent1-mcp
```

### Mode HTTP (assistant distant, API)

Pour un assistant qui ne tourne pas sur votre machine, lancez le serveur en **Streamable HTTP** derrière un reverse proxy HTTPS :

```bash
export ENT1_TRANSPORT=http
export ENT1_ECOUTE=0.0.0.0:8080
export ENT1_HOTES=ent-mcp.mondomaine.fr
export ENT1_MCP_JETON="$(openssl rand -hex 32)"
ent1-mcp
```

Le point d'accès est `https://ent-mcp.mondomaine.fr/mcp`, protégé par l'en-tête `Authorization: Bearer <jeton>`. Une sonde de santé répond sur `/sante`.

Exemple avec le connecteur MCP de l'API Claude :

```json
"mcp_servers": [{
  "type": "url",
  "url": "https://ent-mcp.mondomaine.fr/mcp",
  "name": "ent",
  "authorization_token": "<jeton>"
}]
```

### claude.ai et ChatGPT (web et mobile)

Les connecteurs personnalisés de claude.ai et de ChatGPT n'acceptent pas de jeton fixe : ils exigent **OAuth 2.1**. Le mode HTTP actuel ne suffit donc pas pour eux. C'est la prochaine étape du projet (voir la [feuille de route](#feuille-de-route)) : une page de connexion où chaque parent saisit ses identifiants ENT, sans jamais les confier à l'assistant.

## Sécurité et vie privée

L'ENT contient des données d'enfants. Ce projet part de ce principe.

**Ce que fait le serveur**

- Il ne stocke rien : ni message, ni document, ni identifiant sur disque. La session ENT (cookie) vit en mémoire et disparaît à l'arrêt.
- Le mot de passe n'est lu que dans l'environnement et reste enveloppé dans un type [`secrecy`](https://docs.rs/secrecy) : il n'apparaît ni dans les journaux, ni dans les messages d'erreur, ni dans `Debug`.
- Si l'ENT refuse les identifiants, le serveur **cesse toute tentative** jusqu'à son redémarrage, pour ne pas faire verrouiller votre compte.
- Les reconnexions sont sérialisées : dix requêtes qui trouvent la session expirée déclenchent une seule connexion.
- Chaque identifiant fourni par l'assistant est vérifié avant d'être placé dans une URL : impossible de détourner un appel vers une autre route de l'ENT.
- Toute écriture porte le jeton anti-CSRF de l'ENT (`X-XSRF-TOKEN`), comme l'application web.
- En HTTP : jeton obligatoire comparé en temps constant, protection contre le *DNS rebinding* (liste d'hôtes autorisés), `unsafe` interdit dans tout le code.

**Ce que vous devez savoir**

- Les contenus lus par l'assistant (messages, mots, noms d'enfants et d'enseignants) sont envoyés au fournisseur de l'assistant (Anthropic, OpenAI…). Lisez sa politique de confidentialité et, si possible, désactivez l'utilisation de vos conversations pour l'entraînement.
- L'assistant agit en votre nom. Demandez-lui de **toujours vous montrer un message avant de l'envoyer** ; les instructions du serveur le lui rappellent, et la plupart des clients demandent une confirmation avant chaque outil d'écriture.
- N'exposez jamais le mode HTTP sans HTTPS ni sans jeton.

Une faille ? Merci de la signaler en privé (voir [SECURITY.md](SECURITY.md)) plutôt que dans une issue publique.

## Autres ENT Edifice

L'ENT ONE est une déclinaison de la plateforme open source Edifice, également utilisée sous d'autres noms (NEO, ONE dans d'autres académies, ENT de collectivités…). Les routes utilisées étant celles du socle commun, `ent1-mcp` devrait fonctionner ailleurs en changeant `ENT1_URL`. Seule l'instance de La Réunion (`ent1d.ac-reunion.fr`) a été testée : vos retours sont bienvenus.

Les ENT qui imposent une connexion **ÉduConnect** ou une double authentification ne sont pas pris en charge pour l'instant (voir [limites](#limites-connues)).

## Architecture

```
  Assistant (Claude, ChatGPT…)
          │  MCP : stdio ou Streamable HTTP (+ jeton)
          ▼
 ┌──────────────────────────────┐
 │ ent1-mcp                     │
 │  outils/   47 outils MCP     │  un module par application de l'ENT
 │  texte.rs  HTML ⇄ texte      │
 │  ent/      client HTTP       │  session, reconnexion, XSRF, identifiants typés
 └──────────────┬───────────────┘
                │  HTTPS, cookie de session
                ▼
        ENT ONE / Edifice
   (/conversation, /schoolbook, /homeworks, /actualites, /blog, /rack…)
```

- **Connexion** : formulaire `POST /auth/login`, puis cookie de session. Une réponse 401 ou une redirection vers la page de connexion déclenche une reconnexion et un unique rejeu de la requête.
- **Un compte par processus** : le client ENT est partagé par toutes les sessions MCP du processus. Pour servir plusieurs familles, lancez un processus par compte (ou attendez le mode OAuth).
- **Stack** : [`rmcp`](https://crates.io/crates/rmcp) (SDK MCP officiel), `reqwest` + rustls, `axum`, `tokio`, `schemars`, `html2text`, `jiff`.

## Développement

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings   # lints pedantic activés dans Cargo.toml
cargo fmt --check
```

### Vérifier contre un vrai ENT

Le script [`scripts/verifier.py`](scripts/verifier.py) lance le serveur, appelle tous les outils **de lecture** et affiche un résumé (nombre d'éléments renvoyés, erreurs) sans afficher les contenus. Il n'écrit rien dans l'ENT.

```bash
cargo build --release
export ENT1_IDENTIFIANT=prenom.nom
read -rs ENT1_MOT_DE_PASSE && export ENT1_MOT_DE_PASSE
python3 scripts/verifier.py
```

### Explorer avec l'inspecteur MCP

```bash
npx @modelcontextprotocol/inspector target/release/ent1-mcp
```

### Ajouter un outil

1. Repérer l'appel réseau dans l'application web de l'ENT (outils de développement du navigateur, onglet Réseau).
2. Ajouter la méthode dans le module `src/outils/<application>.rs` correspondant, avec `#[tool(...)]`, une description en français claire pour l'assistant et les bonnes annotations (`read_only_hint`, `destructive_hint`…).
3. Construire les chemins avec `ent::chemin(&[...])`, qui refuse les identifiants dangereux.
4. Les sorties sont des structures `Serialize + JsonSchema` en français.

## Limites connues

- **ÉduConnect, FranceConnect et double authentification** : seule la connexion par identifiant et mot de passe de l'ENT est prise en charge.
- **Premières connexions** : si l'ENT exige une action (acceptation de nouvelles CGU, changement de mot de passe), le serveur le signale ; faites-la une fois dans le navigateur.
- **Formulaires** : lecture des questions et lien de réponse ; la saisie des réponses se fait dans le navigateur.
- **Calendrier, wiki, cahier de texte** : lecture seule (le parent n'y écrit pas).
- **API non documentée** : une mise à jour de l'ENT peut casser un outil. Le script de vérification permet de le détecter rapidement.

## Feuille de route

- [ ] **OAuth 2.1** pour claude.ai et ChatGPT : serveur d'autorisation intégré (enregistrement dynamique des clients, PKCE), page de connexion ENT, une session ENT par parent.
- [ ] Réponse aux formulaires depuis l'assistant.
- [ ] Création d'événements dans son calendrier personnel.
- [ ] Élicitation MCP : confirmation native avant chaque envoi.
- [ ] Binaires précompilés (macOS, Linux, Windows) et image Docker publiée.
- [ ] Tests d'intégration sur un serveur ENT simulé.

## Contribuer

Les contributions sont bienvenues, en français : issues, retours d'autres académies, nouveaux outils, corrections de la documentation. Voir [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

Double licence, au choix : [MIT](LICENSE-MIT) ou [Apache 2.0](LICENSE-APACHE).

« ONE », « Edifice » et les noms d'académies appartiennent à leurs propriétaires respectifs. Ce projet n'y est pas affilié.
