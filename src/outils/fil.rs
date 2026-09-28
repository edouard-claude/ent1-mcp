use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ent1Server, Sortie, page_defaut};
use crate::ent::model::Notifications;
use crate::texte::html_vers_texte;

/// Catégories du fil de nouveautés.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum TypeNouveaute {
    Messagerie,
    CarnetDeLiaison,
    CahierDeTexte,
    Actualites,
    Blog,
    Calendrier,
    Casier,
    Formulaire,
    Wiki,
    EspaceDocumentaire,
}

impl TypeNouveaute {
    fn code(self) -> &'static str {
        match self {
            Self::Messagerie => "MESSAGERIE",
            Self::CarnetDeLiaison => "SCHOOLBOOK",
            Self::CahierDeTexte => "HOMEWORKS",
            Self::Actualites => "NEWS",
            Self::Blog => "BLOG",
            Self::Calendrier => "CALENDAR",
            Self::Casier => "RACK",
            Self::Formulaire => "FORMULAIRE",
            Self::Wiki => "WIKI",
            Self::EspaceDocumentaire => "WORKSPACE",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FilParams {
    /// Catégories à inclure. Vide = toutes.
    #[serde(default)]
    types: Vec<TypeNouveaute>,
    /// Page (0 = la plus récente).
    #[serde(default = "page_defaut")]
    page: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Fil {
    nouveautes: Vec<Nouveaute>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Nouveaute {
    categorie: String,
    texte: String,
    date: Option<String>,
    auteur: Option<String>,
    lien: Option<String>,
}

#[tool_router(router = fil_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Fil de nouveautés de l'ENT (« Quoi de neuf ? ») : nouveaux messages, mots du carnet, \
                       devoirs, actualités, billets de blog… Idéal pour un résumé de ce qui s'est passé.",
        annotations(
            title = "Fil de nouveautés",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn fil_nouveautes(&self, Parameters(p): Parameters<FilParams>) -> Sortie<Fil> {
        let page = p.page.to_string();
        let mut query: Vec<(&str, &str)> = vec![("page", page.as_str())];
        query.extend(p.types.iter().map(|t| ("type", t.code())));
        let n: Notifications = self
            .ent
            .get_avec("/timeline/lastNotifications", Some(&query))
            .await?;
        Ok(Json(Fil {
            nouveautes: n
                .results
                .into_iter()
                .map(|n| Nouveaute {
                    categorie: n.nature,
                    texte: html_vers_texte(&n.message),
                    date: self.date_valeur(&n.date),
                    auteur: n.params.username,
                    lien: n.params.resource_uri.map(|u| self.lien(&u)),
                })
                .collect(),
        }))
    }
}
