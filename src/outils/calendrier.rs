use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Ent1Server, Sortie, champ, identifiant};
use crate::ent::chemin;
use crate::texte::html_vers_texte;

#[derive(Debug, Serialize, JsonSchema)]
struct Calendriers {
    calendriers: Vec<CalendrierSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct CalendrierSortie {
    id: String,
    titre: String,
    proprietaire: Option<String>,
    par_defaut: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EvenementsParams {
    /// Calendrier à lire. Absent = tous les calendriers.
    #[serde(default)]
    calendrier_id: Option<String>,
    /// Première date incluse (AAAA-MM-JJ).
    #[serde(default)]
    depuis: Option<String>,
    /// Dernière date incluse (AAAA-MM-JJ).
    #[serde(default)]
    jusqu_au: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Evenements {
    evenements: Vec<EvenementSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct EvenementSortie {
    id: String,
    calendrier_id: String,
    titre: String,
    description: Option<String>,
    lieu: Option<String>,
    debut: Option<String>,
    fin: Option<String>,
    journee_entiere: bool,
}

fn jour(d: &str) -> &str {
    d.get(..10).unwrap_or(d)
}

#[tool_router(router = calendrier_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Calendriers accessibles (calendrier de l'école, de la classe, personnel).",
        annotations(title = "Calendriers", read_only_hint = true, open_world_hint = false)
    )]
    async fn calendrier_lister(&self) -> Sortie<Calendriers> {
        let brut: Vec<Value> = self.ent.get("/calendar/calendars").await?;
        Ok(Json(Calendriers {
            calendriers: brut
                .iter()
                .map(|c| CalendrierSortie {
                    id: identifiant(c),
                    titre: champ(c, &["title"]).unwrap_or_default(),
                    proprietaire: c.get("owner").and_then(|o| champ(o, &["displayName"])),
                    par_defaut: c
                        .get("is_default")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Événements des calendriers (réunions, sorties, vacances…), éventuellement entre deux dates.",
        annotations(title = "Événements", read_only_hint = true, open_world_hint = false)
    )]
    async fn calendrier_evenements(
        &self,
        Parameters(p): Parameters<EvenementsParams>,
    ) -> Sortie<Evenements> {
        let ids: Vec<String> = if let Some(id) = p.calendrier_id {
            vec![id]
        } else {
            let brut: Vec<Value> = self.ent.get("/calendar/calendars").await?;
            brut.iter()
                .map(identifiant)
                .filter(|i| !i.is_empty())
                .collect()
        };
        let depuis = p.depuis.as_deref().map(jour);
        let jusqu_au = p.jusqu_au.as_deref().map(jour);
        let mut evenements = Vec::new();
        for cal in ids {
            let c = chemin(&["/calendar/", &cal, "/events"])?;
            let brut: Vec<Value> = self.ent.get(&c).await?;
            for e in &brut {
                let debut = champ(e, &["startMoment", "start"]);
                let fin = champ(e, &["endMoment", "end"]);
                if let (Some(d), Some(deb)) = (jusqu_au, debut.as_deref())
                    && jour(deb) > d
                {
                    continue;
                }
                if let (Some(d), Some(f)) = (depuis, fin.as_deref().or(debut.as_deref()))
                    && jour(f) < d
                {
                    continue;
                }
                evenements.push(EvenementSortie {
                    id: identifiant(e),
                    calendrier_id: cal.clone(),
                    titre: champ(e, &["title"]).unwrap_or_default(),
                    description: champ(e, &["description"]).map(|d| html_vers_texte(&d)),
                    lieu: champ(e, &["location"]),
                    debut,
                    fin,
                    journee_entiere: e.get("allday").and_then(Value::as_bool).unwrap_or(false),
                });
            }
        }
        evenements.sort_by(|a, b| a.debut.cmp(&b.debut));
        Ok(Json(Evenements { evenements }))
    }
}
