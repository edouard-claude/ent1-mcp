# Contribuer à ent1-mcp

Merci ! Le projet s'adresse à une communauté francophone : issues, code, commentaires et documentation sont en français.

## Signaler un problème

Précisez l'adresse de votre ENT (`ENT1_URL`), le type de compte (parent, enseignant…), l'outil concerné et le message d'erreur. Le script `scripts/verifier.py` donne un diagnostic rapide sans exposer de données. **Ne collez jamais de contenu réel** (messages, noms d'enfants) dans une issue.

## Proposer du code

1. Une branche par sujet, des commits courts et explicites.
2. Avant de pousser :

   ```bash
   cargo fmt
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

3. Pour un nouvel outil, suivez la section *Ajouter un outil* du README : description claire pour l'assistant, annotations correctes, chemins construits avec `ent::chemin`, sorties typées.
4. Vérifiez le comportement contre un vrai ENT avec `scripts/verifier.py` et indiquez dans la *pull request* l'instance testée.

## Principes

- Pas de `unwrap()` hors des tests, pas d'`unsafe`.
- Aucune donnée persistée, aucun secret dans les journaux.
- Toute écriture dans l'ENT doit être annotée comme telle (`read_only_hint = false`).
