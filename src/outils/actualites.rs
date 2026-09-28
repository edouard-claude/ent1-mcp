use reqwest::Method;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Ent1Server, Sortie, page_defaut};
use crate::ent::model::Actualite;
use crate::ent::{Corps, chemin};
use crate::texte::html_vers_texte;

fn taille_defaut() -> u32 {
    10
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListerParams {
    /// Ne garder que les actualités de ce fil (voir `actualites_fils`).
    #[serde(default)]
    fil_id: Option<u64>,
    #[serde(default = "page_defaut")]
    page: u32,
    /// Actualités par page (1 à 50, défaut 10).
    #[serde(default = "taille_defaut")]
    taille: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Actualites {
    actualites: Vec<ActualiteSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ActualiteSortie {
    id: Value,
    fil_id: Option<Value>,
    titre: String,
    contenu: String,
    auteur: Option<String>,
    publiee_le: Option<String>,
    a_la_une: bool,
    nb_commentaires: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Fils {
    fils: Vec<FilSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct FilSortie {
    id: Value,
    titre: String,
    etablissement: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdParams {
    /// Identifiant (numérique) de l'actualité.
    id: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ActualiteDetail {
    actualite: ActualiteSortie,
    commentaires: Vec<CommentaireSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct CommentaireSortie {
    id: Option<Value>,
    auteur: Option<String>,
    texte: String,
    date: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CommenterParams {
    id: u64,
    /// Commentaire en texte brut.
    texte: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Commente {
    fait: bool,
    reponse_ent: Value,
}

impl Ent1Server {
    fn actualite(&self, a: Actualite, brut: &Value) -> ActualiteSortie {
        ActualiteSortie {
            titre: a.title,
            contenu: html_vers_texte(&a.content),
            auteur: a.owner.map(|o| o.display_name),
            publiee_le: a
                .publication_date
                .or(a.created)
                .map(|d| self.date_texte(&d)),
            a_la_une: a.headline,
            nb_commentaires: brut.get("numberOfComments").and_then(Value::as_u64),
            fil_id: brut.get("threadId").cloned(),
            id: a.id,
        }
    }

    fn commentaires(&self, v: Value) -> Vec<CommentaireSortie> {
        let Value::Array(liste) = v else {
            return Vec::new();
        };
        liste
            .into_iter()
            .map(|c| CommentaireSortie {
                id: c.get("id").or_else(|| c.get("_id")).cloned(),
                auteur: ["username", "owner_name", "authorName", "author"]
                    .iter()
                    .find_map(|k| c.get(*k).and_then(Value::as_str))
                    .map(str::to_owned),
                texte: ["comment", "content", "text"]
                    .iter()
                    .find_map(|k| c.get(*k).and_then(Value::as_str))
                    .map(html_vers_texte)
                    .unwrap_or_default(),
                date: ["created", "modified", "date"]
                    .iter()
                    .find_map(|k| c.get(*k))
                    .and_then(|d| self.date_valeur(d)),
            })
            .collect()
    }
}

#[tool_router(router = actualites_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Dernières actualités publiées par l'école (sorties, réunions, informations générales).",
        annotations(title = "Actualités", read_only_hint = true, open_world_hint = false)
    )]
    async fn actualites_lister(
        &self,
        Parameters(p): Parameters<ListerParams>,
    ) -> Sortie<Actualites> {
        let page = p.page.to_string();
        let taille = p.taille.clamp(1, 50).to_string();
        let query = [("page", page.as_str()), ("pageSize", taille.as_str())];
        let brut: Vec<Value> = self
            .ent
            .get_avec("/actualites/api/v1/infos", Some(&query))
            .await?;
        let mut actualites = Vec::with_capacity(brut.len());
        for v in brut {
            if let Some(fil) = p.fil_id
                && v.get("threadId").and_then(Value::as_u64) != Some(fil)
            {
                continue;
            }
            let a: Actualite = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
            actualites.push(self.actualite(a, &v));
        }
        Ok(Json(Actualites { actualites }))
    }

    #[tool(
        description = "Fils d'actualités disponibles (un par école ou par thème).",
        annotations(
            title = "Fils d'actualités",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn actualites_fils(&self) -> Sortie<Fils> {
        let brut: Vec<Value> = self.ent.get("/actualites/api/v1/threads").await?;
        Ok(Json(Fils {
            fils: brut
                .into_iter()
                .map(|f| FilSortie {
                    id: f.get("id").cloned().unwrap_or(Value::Null),
                    titre: f
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    etablissement: f
                        .get("structure")
                        .and_then(|s| s.get("name").or(Some(s)))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Lit une actualité en entier, avec ses commentaires.",
        annotations(
            title = "Lire une actualité",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn actualites_lire(
        &self,
        Parameters(p): Parameters<IdParams>,
    ) -> Sortie<ActualiteDetail> {
        let id = p.id.to_string();
        let c = chemin(&["/actualites/api/v1/infos/", &id])?;
        let v: Value = self.ent.get(&c).await?;
        let a: Actualite = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
        let c = chemin(&["/actualites/api/v1/infos/", &id, "/comments"])?;
        let commentaires: Value = self.ent.get(&c).await?;
        Ok(Json(ActualiteDetail {
            actualite: self.actualite(a, &v),
            commentaires: self.commentaires(commentaires),
        }))
    }

    #[tool(
        description = "Publie un commentaire sous une actualité (si l'école autorise les commentaires). \
                       Faire valider le texte par l'utilisateur.",
        annotations(
            title = "Commenter une actualité",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn actualites_commenter(
        &self,
        Parameters(p): Parameters<CommenterParams>,
    ) -> Sortie<Commente> {
        let id = p.id.to_string();
        let c = chemin(&["/actualites/api/v1/infos/", &id, "/comments"])?;
        let corps = json!({ "info_id": p.id, "comment": p.texte });
        let v: Value = self
            .ent
            .ecrire(Method::POST, &c, Corps::Json(&corps))
            .await?;
        Ok(Json(Commente {
            fait: true,
            reponse_ent: v,
        }))
    }
}
