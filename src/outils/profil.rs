use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::Serialize;

use super::{Ent1Server, Sortie};
use crate::ent::ids::UtilisateurId;
use crate::ent::model::UserInfo;

#[derive(Debug, Serialize, JsonSchema)]
struct Profil {
    id: UtilisateurId,
    nom: String,
    /// Type de compte ENT (ex. `PERSRELELEVE` = parent, `ENSEIGNANT`, `ELEVE`).
    type_compte: String,
    enfants: Vec<EnfantSortie>,
    etablissements: Vec<String>,
    classes: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct EnfantSortie {
    id: UtilisateurId,
    prenom: String,
    nom: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Deconnexion {
    deconnecte: bool,
}

impl Ent1Server {
    pub(super) async fn moi(&self) -> Result<UserInfo, String> {
        Ok(self.ent.get("/auth/oauth2/userinfo").await?)
    }
}

#[tool_router(router = profil_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Profil du compte connecté : nom, type de compte, enfants rattachés \
                       (avec leurs identifiants), établissements et classes. À appeler en premier.",
        annotations(title = "Mon profil", read_only_hint = true, open_world_hint = false)
    )]
    async fn profil(&self) -> Sortie<Profil> {
        let moi = self.moi().await?;
        Ok(Json(Profil {
            id: moi.user_id,
            nom: moi.username,
            type_compte: moi.profil,
            enfants: moi
                .children
                .into_iter()
                .map(|(id, e)| EnfantSortie {
                    id: UtilisateurId::from(id.as_str()),
                    prenom: e.first_name,
                    nom: e.last_name,
                })
                .collect(),
            etablissements: moi.structure_names,
            classes: moi.class_names,
        }))
    }

    #[tool(
        description = "Ferme la session ENT en cours. La prochaine action rouvrira une session \
                       automatiquement avec les identifiants configurés.",
        annotations(
            title = "Se déconnecter",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        )
    )]
    async fn deconnexion(&self) -> Sortie<Deconnexion> {
        self.ent.deconnecter().await?;
        Ok(Json(Deconnexion { deconnecte: true }))
    }
}
