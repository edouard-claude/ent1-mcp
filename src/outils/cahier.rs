use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ent1Server, Sortie};
use crate::ent::chemin;
use crate::ent::ids::CahierId;
use crate::ent::model::{Cahier, CahierResume};
use crate::texte::html_vers_texte;

#[derive(Debug, Serialize, JsonSchema)]
struct Cahiers {
    cahiers: Vec<CahierSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct CahierSortie {
    id: CahierId,
    titre: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LireParams {
    id: CahierId,
    /// Première date incluse, au format AAAA-MM-JJ.
    #[serde(default)]
    depuis: Option<String>,
    /// Dernière date incluse, au format AAAA-MM-JJ.
    #[serde(default)]
    jusqu_au: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct CahierLu {
    id: CahierId,
    titre: String,
    jours: Vec<JourSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct JourSortie {
    date: String,
    travaux: Vec<TravailSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct TravailSortie {
    titre: String,
    contenu: String,
}

/// Les 10 premiers caractères d'une date ISO (AAAA-MM-JJ), comparables
/// lexicographiquement.
fn jour(date: &str) -> &str {
    date.get(..10).unwrap_or(date)
}

#[tool_router(router = cahier_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Liste les cahiers de texte (devoirs et travail à faire) accessibles.",
        annotations(
            title = "Cahiers de texte",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn cahier_lister(&self) -> Sortie<Cahiers> {
        let liste: Vec<CahierResume> = self.ent.get("/homeworks/list").await?;
        Ok(Json(Cahiers {
            cahiers: liste
                .into_iter()
                .map(|c| CahierSortie {
                    id: c.id,
                    titre: c.title,
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Lit un cahier de texte : travail à faire, jour par jour, éventuellement entre deux dates.",
        annotations(
            title = "Lire un cahier de texte",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn cahier_lire(&self, Parameters(p): Parameters<LireParams>) -> Sortie<CahierLu> {
        let c = chemin(&["/homeworks/get/", p.id.as_str()])?;
        let cahier: Cahier = self.ent.get(&c).await?;
        let depuis = p.depuis.as_deref().map(jour);
        let jusqu_au = p.jusqu_au.as_deref().map(jour);
        let mut jours: Vec<JourSortie> = cahier
            .data
            .into_iter()
            .filter(|j| depuis.is_none_or(|d| jour(&j.date) >= d))
            .filter(|j| jusqu_au.is_none_or(|f| jour(&j.date) <= f))
            .filter(|j| !j.entries.is_empty())
            .map(|j| JourSortie {
                date: j.date,
                travaux: j
                    .entries
                    .into_iter()
                    .map(|e| TravailSortie {
                        titre: e.title,
                        contenu: html_vers_texte(&e.value),
                    })
                    .collect(),
            })
            .collect();
        jours.sort_by(|a, b| jour(&a.date).cmp(jour(&b.date)));
        Ok(Json(CahierLu {
            id: cahier.id,
            titre: cahier.title,
            jours,
        }))
    }
}
