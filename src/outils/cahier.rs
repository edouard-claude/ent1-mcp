use std::collections::HashSet;

use reqwest::Method;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ent1Server, Sortie};
use crate::ent::ids::CahierId;
use crate::ent::model::{Cahier, CahierResume, StatutEntree};
use crate::ent::{Corps, chemin};
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
    /// Identifiant du travail, pour `cahier_marquer`.
    id: Option<String>,
    titre: String,
    contenu: String,
    /// Marqué comme fait dans l'ENT.
    fait: bool,
}

/// État d'un travail du cahier de texte.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum EtatTravail {
    Fait,
    AFaire,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MarquerParams {
    cahier_id: CahierId,
    /// Identifiants des travaux (champ `id` renvoyé par `cahier_lire`).
    travaux: Vec<String>,
    etat: EtatTravail,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Marques {
    marques: usize,
    /// Travaux désormais faits dans ce cahier, selon l'ENT.
    faits: Vec<String>,
}

/// Les 10 premiers caractères d'une date ISO (AAAA-MM-JJ), comparables
/// lexicographiquement.
fn jour(date: &str) -> &str {
    date.get(..10).unwrap_or(date)
}

impl Ent1Server {
    /// Identifiants des travaux marqués faits dans un cahier.
    async fn travaux_faits(&self, cahier: &CahierId) -> Result<HashSet<String>, String> {
        let c = chemin(&["/homeworks/", cahier.as_str(), "/entry/status"])?;
        let statuts: Vec<StatutEntree> = self.ent.get(&c).await?;
        Ok(statuts
            .into_iter()
            .filter(|s| s.finished)
            .map(|s| s.entry_id)
            .collect())
    }
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
        description = "Lit un cahier de texte : travail à faire, jour par jour, éventuellement entre deux dates, \
                       avec pour chaque travail son identifiant et s'il est marqué fait.",
        annotations(
            title = "Lire un cahier de texte",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn cahier_lire(&self, Parameters(p): Parameters<LireParams>) -> Sortie<CahierLu> {
        let c = chemin(&["/homeworks/get/", p.id.as_str()])?;
        let cahier: Cahier = self.ent.get(&c).await?;
        let faits = self.travaux_faits(&p.id).await?;
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
                        fait: e.id.as_ref().is_some_and(|id| faits.contains(id)),
                        id: e.id,
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

    #[tool(
        description = "Marque des travaux du cahier de texte comme faits (ou à refaire), comme la case à cocher \
                       de l'ENT. Les identifiants viennent de `cahier_lire`.",
        annotations(
            title = "Marquer un devoir fait",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn cahier_marquer(&self, Parameters(p): Parameters<MarquerParams>) -> Sortie<Marques> {
        if p.travaux.is_empty() {
            return Err("aucun travail fourni".into());
        }
        let c = chemin(&["/homeworks/", p.cahier_id.as_str(), "/entry/status"])?;
        let fini = matches!(p.etat, EtatTravail::Fait);
        for travail in &p.travaux {
            let corps = serde_json::json!({ "entryid": travail, "finished": fini });
            self.ent
                .ecrire_sans_retour(Method::PUT, &c, Corps::Json(&corps))
                .await?;
        }
        Ok(Json(Marques {
            marques: p.travaux.len(),
            faits: self
                .travaux_faits(&p.cahier_id)
                .await?
                .into_iter()
                .collect(),
        }))
    }
}
