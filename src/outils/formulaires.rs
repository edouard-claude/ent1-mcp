use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Ent1Server, Sortie, champ};
use crate::ent::chemin;
use crate::texte::html_vers_texte;

#[derive(Debug, Serialize, JsonSchema)]
struct Formulaires {
    formulaires: Vec<FormulaireSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct FormulaireSortie {
    formulaire_id: Option<u64>,
    titre: Option<String>,
    envoye_par: Option<String>,
    /// `TO_DO` (à remplir), `ON_CHANGE` (en cours) ou `FINISHED`.
    statut: Option<String>,
    envoye_le: Option<String>,
    repondu_le: Option<String>,
    date_limite: Option<String>,
    lien: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LireParams {
    formulaire_id: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct FormulaireDetail {
    titre: Option<String>,
    description: Option<String>,
    auteur: Option<String>,
    ouverture: Option<String>,
    date_limite: Option<String>,
    anonyme: bool,
    questions: Vec<QuestionSortie>,
    /// Lien pour répondre dans le navigateur.
    lien: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct QuestionSortie {
    id: Option<u64>,
    position: Option<u64>,
    titre: Option<String>,
    enonce: Option<String>,
    obligatoire: bool,
    /// Type ENT (1 = texte libre court, 2 = long, 4 = choix unique, 5 = choix multiples, 6 = date…).
    type_question: Option<u64>,
    choix: Vec<String>,
}

impl Ent1Server {
    fn lien_formulaire(&self, id: u64) -> String {
        self.lien(&format!("/formulaire#/form/{id}"))
    }
}

#[tool_router(router = formulaires_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Formulaires et sondages reçus (autorisations de sortie, sondages de l'école…) avec leur statut.",
        annotations(
            title = "Formulaires reçus",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn formulaires_lister(&self) -> Sortie<Formulaires> {
        let distributions: Vec<Value> = self.ent.get("/formulaire/distributions/listMine").await?;
        let mut formulaires = Vec::with_capacity(distributions.len());
        for d in &distributions {
            let id = d.get("form_id").and_then(Value::as_u64);
            // Le titre vit sur le formulaire ; un formulaire supprimé ne bloque pas la liste.
            let form: Option<Value> = match id {
                Some(id) => self.ent.get(&format!("/formulaire/forms/{id}")).await.ok(),
                None => None,
            };
            formulaires.push(FormulaireSortie {
                formulaire_id: id,
                titre: form.as_ref().and_then(|f| champ(f, &["title"])),
                envoye_par: champ(d, &["sender_name"]),
                statut: champ(d, &["status"]),
                envoye_le: d.get("date_sending").and_then(|v| self.date_valeur(v)),
                repondu_le: d.get("date_response").and_then(|v| self.date_valeur(v)),
                date_limite: form
                    .as_ref()
                    .and_then(|f| f.get("date_ending"))
                    .and_then(|v| self.date_valeur(v)),
                lien: id.map(|i| self.lien_formulaire(i)),
            });
        }
        Ok(Json(Formulaires { formulaires }))
    }

    #[tool(
        description = "Détail d'un formulaire : description, questions et choix possibles, avec le lien pour y répondre.",
        annotations(
            title = "Lire un formulaire",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn formulaires_lire(
        &self,
        Parameters(p): Parameters<LireParams>,
    ) -> Sortie<FormulaireDetail> {
        let id = p.formulaire_id.to_string();
        let f: Value = self.ent.get(&chemin(&["/formulaire/forms/", &id])?).await?;
        let questions: Vec<Value> = self
            .ent
            .get(&chemin(&["/formulaire/forms/", &id, "/questions"])?)
            .await?;
        Ok(Json(FormulaireDetail {
            titre: champ(&f, &["title"]),
            description: champ(&f, &["description"]).map(|d| html_vers_texte(&d)),
            auteur: champ(&f, &["owner_name"]),
            ouverture: f.get("date_opening").and_then(|v| self.date_valeur(v)),
            date_limite: f.get("date_ending").and_then(|v| self.date_valeur(v)),
            anonyme: f.get("anonymous").and_then(Value::as_bool).unwrap_or(false),
            questions: questions
                .iter()
                .map(|q| QuestionSortie {
                    id: q.get("id").and_then(Value::as_u64),
                    position: q.get("position").and_then(Value::as_u64),
                    titre: champ(q, &["title"]),
                    enonce: champ(q, &["statement"]).map(|s| html_vers_texte(&s)),
                    obligatoire: q.get("mandatory").and_then(Value::as_bool).unwrap_or(false),
                    type_question: q.get("question_type").and_then(Value::as_u64),
                    choix: q
                        .get("choices")
                        .and_then(Value::as_array)
                        .map(|l| l.iter().filter_map(|c| champ(c, &["value"])).collect())
                        .unwrap_or_default(),
                })
                .collect(),
            lien: self.lien_formulaire(p.formulaire_id),
        }))
    }
}
