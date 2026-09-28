use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Ent1Server, Sortie, champ, identifiant};
use crate::ent::chemin;
use crate::texte::html_vers_texte;

#[derive(Debug, Serialize, JsonSchema)]
struct Wikis {
    wikis: Vec<WikiSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct WikiSortie {
    id: String,
    titre: String,
    pages: Vec<PageResume>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct PageResume {
    id: String,
    titre: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageParams {
    wiki_id: String,
    page_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Page {
    titre: String,
    contenu: String,
    auteur: Option<String>,
    modifiee_le: Option<String>,
}

#[tool_router(router = wiki_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Wikis accessibles et la liste de leurs pages.",
        annotations(title = "Wikis", read_only_hint = true, open_world_hint = false)
    )]
    async fn wiki_lister(&self) -> Sortie<Wikis> {
        let brut: Vec<Value> = self.ent.get("/wiki/listallpages?visible=true").await?;
        Ok(Json(Wikis {
            wikis: brut
                .iter()
                .map(|w| WikiSortie {
                    id: identifiant(w),
                    titre: champ(w, &["title"]).unwrap_or_default(),
                    pages: w
                        .get("pages")
                        .and_then(Value::as_array)
                        .map(|l| {
                            l.iter()
                                .map(|p| PageResume {
                                    id: identifiant(p),
                                    titre: champ(p, &["title"]).unwrap_or_default(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Lit une page de wiki.",
        annotations(
            title = "Lire une page de wiki",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn wiki_lire_page(&self, Parameters(p): Parameters<PageParams>) -> Sortie<Page> {
        let c = chemin(&["/wiki/", &p.wiki_id, "/page/", &p.page_id])?;
        let v: Value = self.ent.get(&c).await?;
        // Selon la version, la page est renvoyée seule ou dans `pages[0]`.
        let page = v
            .get("pages")
            .and_then(Value::as_array)
            .and_then(|l| l.first())
            .unwrap_or(&v);
        Ok(Json(Page {
            titre: champ(page, &["title"]).unwrap_or_default(),
            contenu: html_vers_texte(&champ(page, &["content"]).unwrap_or_default()),
            auteur: champ(page, &["authorName", "lastContributerName"]),
            modifiee_le: page.get("modified").and_then(|d| self.date_valeur(d)),
        }))
    }
}
