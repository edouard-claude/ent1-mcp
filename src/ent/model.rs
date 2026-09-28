//! Formes brutes des réponses JSON de l'ENT (plateforme Edifice / ODE).
//!
//! Volontairement tolérantes : champs optionnels et valeurs par défaut,
//! car l'API n'est ni documentée ni versionnée. Aucun `deny_unknown_fields`.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::ids::{CahierId, DestinataireId, MessageId, MotId, UtilisateurId};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub user_id: UtilisateurId,
    #[serde(default)]
    pub username: String,
    #[serde(rename = "type", default)]
    pub profil: String,
    #[serde(default)]
    pub children: BTreeMap<String, Enfant>,
    #[serde(default)]
    pub structure_names: Vec<String>,
    #[serde(default)]
    pub class_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Enfant {
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Personne {
    pub id: DestinataireId,
    #[serde(default, alias = "name")]
    pub display_name: String,
    #[serde(default)]
    pub profile: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Destinataires {
    #[serde(default)]
    pub users: Vec<Personne>,
    #[serde(default)]
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
    #[serde(default)]
    pub to: Destinataires,
    #[serde(default)]
    pub date: Option<i64>,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
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
    #[serde(default)]
    pub to: Destinataires,
    #[serde(default)]
    pub cc: Destinataires,
    #[serde(default)]
    pub date: Option<i64>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
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
    #[serde(default)]
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
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub sending_date: Option<String>,
    /// Le mot accepte-t-il une réponse du parent ?
    #[serde(default)]
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
    #[serde(default)]
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct Cahier {
    #[serde(rename = "_id")]
    pub id: CahierId,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub data: Vec<JourCahier>,
}

#[derive(Debug, Deserialize)]
pub struct JourCahier {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub entries: Vec<EntreeCahier>,
}

#[derive(Debug, Deserialize)]
pub struct EntreeCahier {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Actualite {
    pub id: Value,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub owner: Option<Personne>,
    #[serde(default)]
    pub publication_date: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub headline: bool,
}

#[derive(Debug, Deserialize)]
pub struct Notifications {
    #[serde(default)]
    pub results: Vec<Notification>,
}

#[derive(Debug, Deserialize)]
pub struct Notification {
    #[serde(rename = "type", default)]
    pub nature: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub date: Value,
    #[serde(default)]
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
