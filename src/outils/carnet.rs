use reqwest::Method;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Ent1Server, Sortie, page_defaut};
use crate::ent::ids::{MotId, UtilisateurId};
use crate::ent::model::Mot;
use crate::ent::{Corps, chemin};
use crate::texte::html_vers_texte;

#[derive(Debug, Deserialize, JsonSchema)]
struct ListerParams {
    /// Enfant concerné (voir `profil`). Absent = tous les enfants.
    #[serde(default)]
    enfant_id: Option<UtilisateurId>,
    #[serde(default = "page_defaut")]
    page: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Mots {
    mots: Vec<MotSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct MotSortie {
    id: MotId,
    enfant_id: UtilisateurId,
    enfant: String,
    titre: String,
    texte: String,
    date: Option<String>,
    auteur: Option<String>,
    categorie: Option<String>,
    /// Le mot attend-il une réponse écrite ?
    reponse_possible: bool,
    /// Avez-vous déjà signé (accusé de lecture) ce mot ?
    signe_par_moi: bool,
    signe_par: Vec<String>,
    reponses: Vec<ReponseSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReponseSortie {
    auteur: Option<String>,
    texte: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SignerParams {
    mot_id: MotId,
    enfant_id: UtilisateurId,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RepondreParams {
    mot_id: MotId,
    enfant_id: UtilisateurId,
    /// Réponse en texte brut.
    texte: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Resultat {
    fait: bool,
    reponse_ent: Value,
}

#[tool_router(router = carnet_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Mots du carnet de liaison (du plus récent au plus ancien), avec leur état de signature \
                       et les réponses déjà données.",
        annotations(
            title = "Carnet de liaison",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn carnet_lister(&self, Parameters(p): Parameters<ListerParams>) -> Sortie<Mots> {
        let moi = self.moi().await?;
        let enfants: Vec<(UtilisateurId, String)> = moi
            .children
            .into_iter()
            .map(|(id, e)| {
                (
                    UtilisateurId::from(id.as_str()),
                    format!("{} {}", e.first_name, e.last_name),
                )
            })
            .filter(|(id, _)| p.enfant_id.as_ref().is_none_or(|voulu| voulu == id))
            .collect();
        if enfants.is_empty() {
            return Err("aucun enfant correspondant sur ce compte (voir `profil`)".into());
        }

        let page = p.page.to_string();
        let mut mots = Vec::new();
        for (enfant_id, enfant) in enfants {
            let c = chemin(&["/schoolbook/list/", &page, "/", enfant_id.as_str()])?;
            let liste: Vec<Mot> = self.ent.get(&c).await?;
            for m in liste {
                let accuses = m.acknowledgments.unwrap_or_default();
                mots.push(MotSortie {
                    id: m.id,
                    enfant_id: enfant_id.clone(),
                    enfant: enfant.clone(),
                    titre: m.title,
                    texte: html_vers_texte(&m.text),
                    date: m.sending_date,
                    auteur: m.owner_name,
                    categorie: m.category,
                    reponse_possible: m.reply,
                    signe_par_moi: accuses
                        .iter()
                        .any(|a| a.owner.as_ref() == Some(&moi.user_id)),
                    signe_par: accuses.into_iter().filter_map(|a| a.parent_name).collect(),
                    reponses: m
                        .responses
                        .unwrap_or_default()
                        .into_iter()
                        .map(|r| ReponseSortie {
                            auteur: r.parent_name,
                            texte: r.comment.map(|c| html_vers_texte(&c)),
                        })
                        .collect(),
                });
            }
        }
        Ok(Json(Mots { mots }))
    }

    #[tool(
        description = "Signe un mot du carnet de liaison (accusé de lecture) pour un enfant.",
        annotations(
            title = "Signer un mot",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn carnet_signer(&self, Parameters(p): Parameters<SignerParams>) -> Sortie<Resultat> {
        let mot = p.mot_id.to_string();
        let c = chemin(&[
            "/schoolbook/relation/acknowledge/",
            &mot,
            "/",
            p.enfant_id.as_str(),
        ])?;
        let v: Value = self
            .ent
            .ecrire::<Value, ()>(Method::POST, &c, Corps::Aucun)
            .await?;
        Ok(Json(Resultat {
            fait: true,
            reponse_ent: v,
        }))
    }

    #[tool(
        description = "Répond par écrit à un mot du carnet de liaison (si le mot accepte les réponses). \
                       Faire valider le texte par l'utilisateur avant l'envoi.",
        annotations(
            title = "Répondre à un mot",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn carnet_repondre(&self, Parameters(p): Parameters<RepondreParams>) -> Sortie<Resultat> {
        let mot = p.mot_id.to_string();
        let c = chemin(&["/schoolbook/relation/reply/", &mot])?;
        let corps = json!({ "text": p.texte, "studentId": p.enfant_id });
        let v: Value = self
            .ent
            .ecrire(Method::POST, &c, Corps::Json(&corps))
            .await?;
        Ok(Json(Resultat {
            fait: true,
            reponse_ent: v,
        }))
    }
}
