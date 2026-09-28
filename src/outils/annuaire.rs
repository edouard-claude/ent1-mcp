use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Ent1Server, Sortie};
use crate::ent::ids::{DestinataireId, segment_sur};
use crate::ent::model::Visible;

#[derive(Debug, Deserialize, JsonSchema)]
struct RechercheParams {
    /// Nom, prénom, classe ou fonction (ex. « Martin », « CM1 », « directeur »). Vide = tous les contacts autorisés.
    #[serde(default)]
    recherche: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Contacts {
    contacts: Vec<Contact>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Contact {
    /// Identifiant à utiliser comme destinataire (`messagerie_envoyer`).
    id: DestinataireId,
    nom: String,
    /// `Teacher`, `Personnel`, `Relative`, `Student`… ou vide pour un groupe.
    profil: Option<String>,
    etablissement: Option<String>,
    /// `User`, `Group`, `ShareBookmark` ou `BroadcastGroup`.
    nature: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FicheParams {
    /// Identifiant de l'utilisateur.
    id: DestinataireId,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Fiche {
    /// Fiche annuaire telle que renvoyée par l'ENT (nom, fonction, établissements…).
    fiche: Value,
}

#[tool_router(router = annuaire_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Recherche dans l'annuaire les personnes et groupes que vous avez le droit de contacter \
                       (enseignants, direction, personnels, parents, groupes de classe). \
                       Renvoie les identifiants nécessaires pour écrire un message.",
        annotations(
            title = "Rechercher un contact",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn annuaire_rechercher(
        &self,
        Parameters(p): Parameters<RechercheParams>,
    ) -> Sortie<Contacts> {
        let query = [("query", p.recherche.trim())];
        let visibles: Vec<Visible> = self
            .ent
            .get_avec("/communication/visible/search", Some(&query))
            .await?;
        Ok(Json(Contacts {
            contacts: visibles
                .into_iter()
                .map(|v| Contact {
                    id: v.id,
                    nom: v.display_name,
                    profil: v.profile,
                    etablissement: v.structure_name,
                    nature: v.nature,
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Fiche annuaire détaillée d'une personne (fonction, établissements, informations publiques).",
        annotations(
            title = "Fiche annuaire",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn annuaire_fiche(&self, Parameters(p): Parameters<FicheParams>) -> Sortie<Fiche> {
        if !segment_sur(p.id.as_str()) {
            return Err("identifiant invalide".into());
        }
        let query = [("id", p.id.as_str())];
        let fiche: Value = self
            .ent
            .get_avec("/userbook/api/person", Some(&query))
            .await?;
        let fiche = match fiche {
            Value::Object(mut o) => o.remove("result").unwrap_or(Value::Object(o)),
            autre => autre,
        };
        Ok(Json(Fiche { fiche }))
    }
}
