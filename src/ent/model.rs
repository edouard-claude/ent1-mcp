//! Formes brutes des réponses JSON de l'ENT (plateforme Edifice / ODE).
//!
//! Volontairement tolérantes : champs optionnels et valeurs par défaut,
//! car l'API n'est ni documentée ni versionnée. Aucun `deny_unknown_fields`.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::ids::{CahierId, DestinataireId, MessageId, MotId, UtilisateurId};

/// `null` → valeur par défaut. L'ENT renvoie `null` là où l'on attendrait
/// une liste vide ou une chaîne vide (ex. `classNames` d'un compte parent) ;
/// `#[serde(default)]` seul ne couvre que les champs absents.
fn nul_ou<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub user_id: UtilisateurId,
    #[serde(default, deserialize_with = "nul_ou")]
    pub username: String,
    #[serde(rename = "type", default, deserialize_with = "nul_ou")]
    pub profil: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub children: BTreeMap<String, Enfant>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub structure_names: Vec<String>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub class_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Enfant {
    #[serde(default, deserialize_with = "nul_ou")]
    pub first_name: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub last_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Personne {
    pub id: DestinataireId,
    #[serde(default, alias = "name", deserialize_with = "nul_ou")]
    pub display_name: String,
    #[serde(default)]
    pub profile: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Destinataires {
    #[serde(default, deserialize_with = "nul_ou")]
    pub users: Vec<Personne>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub groups: Vec<Personne>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageResume {
    pub id: MessageId,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub from: Option<Personne>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub to: Destinataires,
    #[serde(default)]
    pub date: Option<i64>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub unread: bool,
    #[serde(default, deserialize_with = "nul_ou")]
    pub has_attachment: bool,
    #[serde(default)]
    pub state: Option<String>,
    /// Nombre total de messages du dossier (répété sur chaque ligne).
    #[serde(default)]
    pub count: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub id: MessageId,
    #[serde(default)]
    pub parent_id: Option<MessageId>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub from: Option<Personne>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub to: Destinataires,
    #[serde(default, deserialize_with = "nul_ou")]
    pub cc: Destinataires,
    #[serde(default)]
    pub date: Option<i64>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub attachments: Vec<PieceJointe>,
}

#[derive(Debug, Deserialize)]
pub struct PieceJointe {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, alias = "name")]
    pub filename: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Compteur {
    pub count: u64,
}

#[derive(Debug, Deserialize)]
pub struct Cree {
    pub id: MessageId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visible {
    pub id: DestinataireId,
    #[serde(default, deserialize_with = "nul_ou")]
    pub display_name: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub structure_name: Option<String>,
    #[serde(rename = "type", default)]
    pub nature: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Mot {
    pub id: MotId,
    #[serde(default, deserialize_with = "nul_ou")]
    pub title: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub text: String,
    #[serde(default)]
    pub sending_date: Option<String>,
    /// Le mot accepte-t-il une réponse du parent ?
    #[serde(default, deserialize_with = "nul_ou")]
    pub reply: bool,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub owner_name: Option<String>,
    #[serde(default)]
    pub responses: Option<Vec<ReponseMot>>,
    #[serde(default)]
    pub acknowledgments: Option<Vec<Accuse>>,
}

#[derive(Debug, Deserialize)]
pub struct Accuse {
    #[serde(default)]
    pub owner: Option<UtilisateurId>,
    #[serde(default)]
    pub parent_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReponseMot {
    #[serde(default)]
    pub parent_name: Option<String>,
    #[serde(default, alias = "text", alias = "content")]
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CahierResume {
    #[serde(rename = "_id")]
    pub id: CahierId,
    #[serde(default, deserialize_with = "nul_ou")]
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct Cahier {
    #[serde(rename = "_id")]
    pub id: CahierId,
    #[serde(default, deserialize_with = "nul_ou")]
    pub title: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub data: Vec<JourCahier>,
}

#[derive(Debug, Deserialize)]
pub struct JourCahier {
    #[serde(default, deserialize_with = "nul_ou")]
    pub date: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub entries: Vec<EntreeCahier>,
}

#[derive(Debug, Deserialize)]
pub struct EntreeCahier {
    #[serde(rename = "_id", default)]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub title: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Actualite {
    pub id: Value,
    #[serde(default, deserialize_with = "nul_ou")]
    pub title: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub content: String,
    #[serde(default)]
    pub owner: Option<Personne>,
    #[serde(default)]
    pub publication_date: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default, deserialize_with = "nul_ou")]
    pub headline: bool,
}

#[derive(Debug, Deserialize)]
pub struct Notifications {
    #[serde(default, deserialize_with = "nul_ou")]
    pub results: Vec<Notification>,
}

#[derive(Debug, Deserialize)]
pub struct Notification {
    #[serde(rename = "type", default, deserialize_with = "nul_ou")]
    pub nature: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub message: String,
    #[serde(default)]
    pub date: Value,
    #[serde(default, deserialize_with = "nul_ou")]
    pub params: NotificationParams,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationParams {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub resource_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatutEntree {
    pub entry_id: String,
    #[serde(default, deserialize_with = "nul_ou")]
    pub finished: bool,
}

#[cfg(test)]
#[allow(clippy::expect_used)] // tests
mod tests {
    use super::*;

    #[test]
    fn userinfo_tolere_les_null() {
        // Forme réelle d'un compte parent : `classNames` vaut `null`.
        let u: UserInfo = serde_json::from_str(
            r#"{"userId":"u1","username":"X","type":"PERSRELELEVE","children":{"e1":{"firstName":"A","lastName":null}},"structureNames":["Ecole"],"classNames":null}"#,
        )
        .expect("userinfo décodé");
        assert!(u.class_names.is_empty());
        assert_eq!(u.children["e1"].last_name, "");
    }

    #[test]
    fn mot_tolere_les_null() {
        let m: Mot = serde_json::from_str(
            r#"{"id":1,"title":null,"text":"t","sending_date":"2026-09-28","reply":null,"category":"c","owner_name":"o","responses":null,"acknowledgments":null}"#,
        )
        .expect("mot décodé");
        assert!(!m.reply);
        assert_eq!(m.title, "");
    }
}
