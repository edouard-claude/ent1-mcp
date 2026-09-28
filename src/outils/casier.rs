//! Casier (application « Rack ») : dépôt de fichiers d'une personne à une autre.

use std::fmt::Write as _;

use reqwest::Method;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Ent1Server, Fichier, Sortie, champ, fichier_entrant, fichier_sortant, identifiant};
use crate::ent::ids::{DestinataireId, segment_sur};
use crate::ent::{Corps, chemin};

#[derive(Debug, Serialize, JsonSchema)]
struct Casier {
    fichiers: Vec<FichierCasier>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct FichierCasier {
    id: String,
    nom: String,
    de: Option<String>,
    a: Option<String>,
    depose_le: Option<String>,
    taille_octets: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DestinatairesParams {
    /// Filtre sur le nom (facultatif).
    #[serde(default)]
    recherche: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Destinataires {
    utilisateurs: Vec<DestinataireCasier>,
    groupes: Vec<DestinataireCasier>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct DestinataireCasier {
    id: String,
    nom: String,
    profil: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DeposerParams {
    /// Identifiants des destinataires (voir `casier_destinataires`).
    destinataires: Vec<DestinataireId>,
    nom_fichier: String,
    contenu_base64: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdParams {
    id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Fait {
    fait: bool,
    reponse_ent: Value,
}

fn destinataires(v: Option<&Value>) -> Vec<DestinataireCasier> {
    v.and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .map(|d| DestinataireCasier {
                    id: identifiant(d),
                    nom: champ(d, &["displayName", "username", "name"]).unwrap_or_default(),
                    profil: champ(d, &["profile"]),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tool_router(router = casier_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Fichiers reçus dans le casier (documents déposés à votre intention par un enseignant ou un parent).",
        annotations(title = "Mon casier", read_only_hint = true, open_world_hint = false)
    )]
    async fn casier_lister(&self) -> Sortie<Casier> {
        let brut: Vec<Value> = self.ent.get("/rack/list").await?;
        Ok(Json(Casier {
            fichiers: brut
                .iter()
                .map(|f| FichierCasier {
                    id: identifiant(f),
                    nom: champ(f, &["name"]).unwrap_or_default(),
                    de: champ(f, &["fromName"]),
                    a: champ(f, &["toName"]),
                    depose_le: f.get("sent").and_then(|d| self.date_valeur(d)),
                    taille_octets: f
                        .get("metadata")
                        .and_then(|m| m.get("size"))
                        .and_then(Value::as_u64),
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Personnes et groupes à qui vous pouvez déposer un fichier dans le casier.",
        annotations(
            title = "Destinataires du casier",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn casier_destinataires(
        &self,
        Parameters(p): Parameters<DestinatairesParams>,
    ) -> Sortie<Destinataires> {
        let c = match p
            .recherche
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
        {
            // Le terme est encodé : il peut contenir espaces et accents.
            Some(r) => format!("/rack/users/available/{}", encoder_segment(r)),
            None => "/rack/users/available".to_owned(),
        };
        let v: Value = self.ent.get(&c).await?;
        Ok(Json(Destinataires {
            utilisateurs: destinataires(v.get("users")),
            groupes: destinataires(v.get("groups")),
        }))
    }

    #[tool(
        description = "Dépose un fichier (base64, 10 Mo maximum) dans le casier d'une ou plusieurs personnes. \
                       Faire valider le fichier et les destinataires par l'utilisateur.",
        annotations(
            title = "Déposer dans un casier",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn casier_deposer(&self, Parameters(p): Parameters<DeposerParams>) -> Sortie<Fait> {
        if p.destinataires.is_empty() {
            return Err("au moins un destinataire est requis".into());
        }
        let octets = fichier_entrant(&p.contenu_base64)?;
        let users = p
            .destinataires
            .iter()
            .map(DestinataireId::as_str)
            .collect::<Vec<_>>()
            .join(",");
        let v: Value = self
            .ent
            .televerser("/rack/", &p.nom_fichier, octets, &[("users", &users)])
            .await?;
        Ok(Json(Fait {
            fait: true,
            reponse_ent: v,
        }))
    }

    #[tool(
        description = "Télécharge un fichier du casier (base64, 10 Mo maximum).",
        annotations(
            title = "Télécharger du casier",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn casier_telecharger(&self, Parameters(p): Parameters<IdParams>) -> Sortie<Fichier> {
        let c = chemin(&["/rack/get/", &p.id])?;
        let (octets, mime) = self.ent.telecharger(&c).await?;
        Ok(Json(fichier_sortant(p.id, &octets, mime)?))
    }

    #[tool(
        description = "Met un fichier du casier à la corbeille.",
        annotations(
            title = "Supprimer du casier",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn casier_supprimer(&self, Parameters(p): Parameters<IdParams>) -> Sortie<Fait> {
        if !segment_sur(&p.id) {
            return Err("identifiant invalide".into());
        }
        let c = chemin(&["/rack/", &p.id, "/trash"])?;
        let v: Value = self
            .ent
            .ecrire(Method::PUT, &c, Corps::Json(&serde_json::json!({})))
            .await?;
        Ok(Json(Fait {
            fait: true,
            reponse_ent: v,
        }))
    }
}

/// Encodage « pourcent » d'un segment de chemin.
fn encoder_segment(s: &str) -> String {
    let mut sortie = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            sortie.push(char::from(b));
        } else {
            let _ = write!(sortie, "%{b:02X}"); // écrire dans une String ne peut pas échouer
        }
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::encoder_segment;

    #[test]
    fn encode_les_segments() {
        assert_eq!(encoder_segment("Mme Hoarau"), "Mme%20Hoarau");
        assert_eq!(encoder_segment("../x"), "..%2Fx");
    }
}
