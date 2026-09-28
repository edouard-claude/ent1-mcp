use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Ent1Server, Fichier, Sortie, champ, fichier_entrant, fichier_sortant, identifiant};
use crate::ent::chemin;
use crate::ent::ids::segment_sur;

/// Vue de l'espace documentaire.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Espace {
    /// Mes documents.
    #[default]
    Mes,
    /// Documents partagés avec moi.
    Partages,
    /// Documents déposés par les applications (pièces jointes, blogs…).
    Applications,
    Corbeille,
}

impl Espace {
    fn filtre(self) -> &'static str {
        match self {
            Self::Mes => "owner",
            Self::Partages => "shared",
            Self::Applications => "protected",
            Self::Corbeille => "trash",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListerParams {
    #[serde(default)]
    espace: Espace,
    /// Dossier à ouvrir (absent = racine).
    #[serde(default)]
    dossier_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Documents {
    elements: Vec<DocumentSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct DocumentSortie {
    id: String,
    nom: String,
    /// `file` ou `folder`.
    nature: Option<String>,
    type_mime: Option<String>,
    taille_octets: Option<u64>,
    proprietaire: Option<String>,
    modifie_le: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdParams {
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DeposerParams {
    nom_fichier: String,
    contenu_base64: String,
    /// Dossier de destination (absent = racine de « Mes documents »).
    #[serde(default)]
    dossier_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Depose {
    id: String,
    reponse_ent: Value,
}

#[tool_router(router = documents_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Contenu de l'espace documentaire (mes documents, partagés avec moi, applications, corbeille).",
        annotations(
            title = "Espace documentaire",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn documents_lister(&self, Parameters(p): Parameters<ListerParams>) -> Sortie<Documents> {
        let mut query = vec![("filter", p.espace.filtre()), ("hierarchical", "true")];
        if let Some(d) = p.dossier_id.as_deref() {
            if !segment_sur(d) {
                return Err("dossier_id invalide".into());
            }
            query.push(("parentId", d));
        }
        let brut: Vec<Value> = self
            .ent
            .get_avec("/workspace/documents", Some(&query))
            .await?;
        Ok(Json(Documents {
            elements: brut
                .iter()
                .map(|d| {
                    let meta = d.get("metadata");
                    DocumentSortie {
                        id: identifiant(d),
                        nom: champ(d, &["name"]).unwrap_or_default(),
                        nature: champ(d, &["eType"]),
                        type_mime: meta.and_then(|m| champ(m, &["content-type"])),
                        taille_octets: meta.and_then(|m| m.get("size")).and_then(Value::as_u64),
                        proprietaire: champ(d, &["ownerName"]),
                        modifie_le: d.get("modified").and_then(|v| self.date_valeur(v)),
                    }
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Télécharge un document de l'espace documentaire (base64, 10 Mo maximum).",
        annotations(
            title = "Télécharger un document",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn documents_telecharger(&self, Parameters(p): Parameters<IdParams>) -> Sortie<Fichier> {
        let c = chemin(&["/workspace/document/", &p.id])?;
        let (octets, mime) = self.ent.telecharger(&c).await?;
        Ok(Json(fichier_sortant(p.id, &octets, mime)?))
    }

    #[tool(
        description = "Dépose un fichier (base64, 10 Mo maximum) dans « Mes documents ».",
        annotations(
            title = "Déposer un document",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn documents_deposer(&self, Parameters(p): Parameters<DeposerParams>) -> Sortie<Depose> {
        let octets = fichier_entrant(&p.contenu_base64)?;
        let c = match p.dossier_id.as_deref() {
            Some(d) => chemin(&["/workspace/document?parentId=", d])?,
            None => "/workspace/document".to_owned(),
        };
        let v: Value = self.ent.televerser(&c, &p.nom_fichier, octets, &[]).await?;
        Ok(Json(Depose {
            id: identifiant(&v),
            reponse_ent: v,
        }))
    }
}
