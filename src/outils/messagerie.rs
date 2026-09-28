use reqwest::Method;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Ent1Server, Fichier, Sortie, fichier_entrant, fichier_sortant, page_defaut};
use crate::ent::ids::{DestinataireId, MessageId, segment_sur};
use crate::ent::model::{Compteur, Cree, Destinataires, Message, MessageResume, Personne};
use crate::ent::{Corps, chemin};
use crate::texte::{html_vers_texte, texte_vers_html};

const TAILLE_PAGE_MAX: u32 = 100;

fn taille_defaut() -> u32 {
    20
}

/// Dossier de la messagerie : un dossier système, ou l'identifiant d'un
/// dossier personnel (voir `messagerie_dossiers`).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Dossier {
    Reception,
    Envoyes,
    Brouillons,
    Corbeille,
    #[serde(untagged)]
    Personnel(String),
}

impl Dossier {
    fn code(&self) -> Result<&str, String> {
        Ok(match self {
            Self::Reception => "inbox",
            Self::Envoyes => "outbox",
            Self::Brouillons => "draft",
            Self::Corbeille => "trash",
            Self::Personnel(id) if segment_sur(id) => id,
            Self::Personnel(id) => return Err(format!("dossier inconnu : {id:?}")),
        })
    }
}

fn dossier_defaut() -> Dossier {
    Dossier::Reception
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListerParams {
    /// `reception` (défaut), `envoyes`, `brouillons`, `corbeille`, ou l'identifiant d'un dossier personnel.
    #[serde(default = "dossier_defaut")]
    dossier: Dossier,
    /// Filtre sur l'objet, l'expéditeur ou les destinataires.
    #[serde(default)]
    recherche: Option<String>,
    /// Ne renvoyer que les messages non lus.
    #[serde(default)]
    non_lus_seulement: bool,
    #[serde(default = "page_defaut")]
    page: u32,
    /// Messages par page (1 à 100, défaut 20).
    #[serde(default = "taille_defaut")]
    taille: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ListeMessages {
    /// Nombre total de messages correspondant, toutes pages confondues.
    total: Option<u64>,
    page: u32,
    messages: Vec<ResumeSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ResumeSortie {
    id: MessageId,
    objet: String,
    expediteur: Option<String>,
    destinataires: Vec<String>,
    date: Option<String>,
    non_lu: bool,
    piece_jointe: bool,
    /// `SENT`, `DRAFT` ou `RECALL` (message rappelé par son auteur).
    etat: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdParams {
    /// Identifiant du message.
    id: MessageId,
}

#[derive(Debug, Serialize, JsonSchema)]
struct MessageSortie {
    id: MessageId,
    objet: String,
    expediteur: Option<PersonneSortie>,
    a: Vec<PersonneSortie>,
    cc: Vec<PersonneSortie>,
    date: Option<String>,
    etat: Option<String>,
    /// Corps du message converti en texte brut.
    corps: String,
    pieces_jointes: Vec<PieceJointeSortie>,
    /// Message auquel celui-ci répond.
    en_reponse_a: Option<MessageId>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct PersonneSortie {
    id: DestinataireId,
    nom: String,
    profil: Option<String>,
}

impl From<Personne> for PersonneSortie {
    fn from(p: Personne) -> Self {
        Self {
            id: p.id,
            nom: p.display_name,
            profil: p.profile,
        }
    }
}

fn personnes(d: Destinataires) -> Vec<PersonneSortie> {
    d.users
        .into_iter()
        .chain(d.groups)
        .map(PersonneSortie::from)
        .collect()
}

#[derive(Debug, Serialize, JsonSchema)]
struct PieceJointeSortie {
    id: Option<String>,
    nom: Option<String>,
    taille_octets: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct NonLus {
    non_lus: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RedactionParams {
    /// Identifiants des destinataires principaux (voir `annuaire_rechercher`).
    a: Vec<DestinataireId>,
    #[serde(default)]
    cc: Vec<DestinataireId>,
    /// Copie cachée.
    #[serde(default)]
    cci: Vec<DestinataireId>,
    objet: String,
    /// Corps du message en texte brut ; les lignes vides séparent les paragraphes.
    corps: String,
    /// Répondre à ce message (conserve le fil de discussion).
    #[serde(default)]
    en_reponse_a: Option<MessageId>,
    /// Transférer ce message (ses pièces jointes sont reprises).
    #[serde(default)]
    transfert_de: Option<MessageId>,
    /// Brouillon existant à compléter puis envoyer / enregistrer.
    #[serde(default)]
    brouillon_id: Option<MessageId>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Envoi {
    id: MessageId,
    envoye: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Brouillon {
    id: MessageId,
}

/// Nouvel état de lecture.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Lecture {
    Lu,
    NonLu,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MarquerParams {
    ids: Vec<MessageId>,
    etat: Lecture,
}

/// Mode de suppression.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Suppression {
    /// Déplacer vers la corbeille (récupérable).
    #[default]
    Corbeille,
    /// Supprimer définitivement des messages déjà dans la corbeille.
    Definitive,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SupprimerParams {
    ids: Vec<MessageId>,
    #[serde(default)]
    mode: Suppression,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdsParams {
    ids: Vec<MessageId>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DeplacerParams {
    ids: Vec<MessageId>,
    /// `reception` ou identifiant d'un dossier personnel.
    vers: Dossier,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Fait {
    fait: bool,
    nombre: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PieceJointeParams {
    message_id: MessageId,
    piece_jointe_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct JoindreParams {
    /// Brouillon auquel joindre le fichier (voir `messagerie_enregistrer_brouillon`).
    brouillon_id: MessageId,
    nom_fichier: String,
    contenu_base64: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct PieceJointeAjoutee {
    id: Value,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Dossiers {
    dossiers: Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CreerDossierParams {
    nom: String,
    /// Dossier parent (facultatif).
    #[serde(default)]
    parent_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct DossierCree {
    id: Value,
}

fn ids_json(ids: &[MessageId]) -> Result<Value, String> {
    if ids.is_empty() {
        return Err("aucun identifiant fourni".into());
    }
    Ok(json!({ "id": ids }))
}

impl Ent1Server {
    /// Crée ou met à jour un brouillon ; renvoie son identifiant.
    async fn brouillon(&self, p: &RedactionParams) -> Result<MessageId, String> {
        if p.a.is_empty() && p.cc.is_empty() && p.cci.is_empty() {
            return Err("au moins un destinataire est requis".into());
        }
        let corps = json!({
            "subject": p.objet,
            "body": texte_vers_html(&p.corps),
            "to": p.a,
            "cc": p.cc,
            "cci": p.cci,
        });
        let id = if let Some(id) = &p.brouillon_id {
            let c = chemin(&["/conversation/draft/", id.as_str()])?;
            self.ent
                .ecrire_sans_retour(Method::PUT, &c, Corps::Json(&corps))
                .await?;
            id.clone()
        } else {
            let mut c = String::from("/conversation/draft");
            if let Some(parent) = &p.en_reponse_a {
                c = chemin(&["/conversation/draft?In-Reply-To=", parent.as_str()])?;
            }
            let cree: Cree = self
                .ent
                .ecrire(Method::POST, &c, Corps::Json(&corps))
                .await?;
            cree.id
        };
        if let Some(original) = &p.transfert_de {
            let c = chemin(&[
                "/conversation/message/",
                id.as_str(),
                "/forward/",
                original.as_str(),
            ])?;
            self.ent
                .ecrire_sans_retour::<()>(Method::PUT, &c, Corps::Aucun)
                .await?;
        }
        Ok(id)
    }
}

#[tool_router(router = messagerie_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Liste les messages d'un dossier de la messagerie (réception par défaut), du plus récent au plus ancien.",
        annotations(
            title = "Lister les messages",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_lister(
        &self,
        Parameters(p): Parameters<ListerParams>,
    ) -> Sortie<ListeMessages> {
        let dossier = p.dossier.code()?;
        let c = chemin(&["/conversation/api/folders/", dossier, "/messages"])?;
        let taille = p.taille.clamp(1, TAILLE_PAGE_MAX).to_string();
        let page = p.page.to_string();
        let mut query = vec![("page", page.as_str()), ("page_size", taille.as_str())];
        if p.non_lus_seulement {
            query.push(("unread", "true"));
        }
        if let Some(r) = p
            .recherche
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
        {
            query.push(("search", r));
        }
        let messages: Vec<MessageResume> = self.ent.get_avec(&c, Some(&query)).await?;
        Ok(Json(ListeMessages {
            total: messages.first().and_then(|m| m.count),
            page: p.page,
            messages: messages
                .into_iter()
                .map(|m| ResumeSortie {
                    objet: m.subject.unwrap_or_default(),
                    expediteur: m.from.map(|f| f.display_name),
                    destinataires: m
                        .to
                        .users
                        .into_iter()
                        .chain(m.to.groups)
                        .map(|d| d.display_name)
                        .collect(),
                    date: m.date.and_then(|d| self.date_ms(d)),
                    non_lu: m.unread,
                    piece_jointe: m.has_attachment,
                    etat: m.state,
                    id: m.id,
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Lit un message complet (corps en texte, destinataires, pièces jointes). \
                       L'ENT le marque alors comme lu.",
        annotations(
            title = "Lire un message",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_lire(&self, Parameters(p): Parameters<IdParams>) -> Sortie<MessageSortie> {
        let c = chemin(&["/conversation/api/messages/", p.id.as_str()])?;
        let m: Message = self.ent.get(&c).await?;
        Ok(Json(MessageSortie {
            objet: m.subject.unwrap_or_default(),
            expediteur: m.from.map(PersonneSortie::from),
            a: personnes(m.to),
            cc: personnes(m.cc),
            date: m.date.and_then(|d| self.date_ms(d)),
            etat: m.state,
            corps: html_vers_texte(m.body.as_deref().unwrap_or_default()),
            pieces_jointes: m
                .attachments
                .into_iter()
                .map(|a| PieceJointeSortie {
                    id: a.id,
                    nom: a.filename,
                    taille_octets: a.size,
                })
                .collect(),
            en_reponse_a: m.parent_id,
            id: m.id,
        }))
    }

    #[tool(
        description = "Nombre de messages non lus dans la boîte de réception.",
        annotations(
            title = "Messages non lus",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_non_lus(&self) -> Sortie<NonLus> {
        let c: Compteur = self
            .ent
            .get_avec("/conversation/api/count/inbox", Some(&[("unread", "true")]))
            .await?;
        Ok(Json(NonLus { non_lus: c.count }))
    }

    #[tool(
        description = "Envoie un message (nouveau, réponse ou transfert). Les destinataires sont des \
                       identifiants obtenus avec `annuaire_rechercher` ou lus dans un message. \
                       Faire valider l'objet, le corps et les destinataires par l'utilisateur avant l'envoi.",
        annotations(
            title = "Envoyer un message",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn messagerie_envoyer(
        &self,
        Parameters(p): Parameters<RedactionParams>,
    ) -> Sortie<Envoi> {
        let id = self.brouillon(&p).await?;
        let mut c = chemin(&["/conversation/send?id=", id.as_str()])?;
        if let Some(parent) = &p.en_reponse_a {
            c.push_str(&chemin(&["&In-Reply-To=", parent.as_str()])?);
        }
        let corps = json!({
            "subject": p.objet,
            "body": texte_vers_html(&p.corps),
            "to": p.a,
            "cc": p.cc,
            "cci": p.cci,
        });
        let _: Value = self
            .ent
            .ecrire(Method::POST, &c, Corps::Json(&corps))
            .await?;
        Ok(Json(Envoi { id, envoye: true }))
    }

    #[tool(
        description = "Crée ou met à jour un brouillon sans l'envoyer (utile pour y joindre des fichiers \
                       avec `messagerie_joindre_fichier`, puis l'envoyer avec `messagerie_envoyer` et `brouillon_id`).",
        annotations(
            title = "Enregistrer un brouillon",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn messagerie_enregistrer_brouillon(
        &self,
        Parameters(p): Parameters<RedactionParams>,
    ) -> Sortie<Brouillon> {
        Ok(Json(Brouillon {
            id: self.brouillon(&p).await?,
        }))
    }

    #[tool(
        description = "Marque des messages comme lus ou non lus.",
        annotations(
            title = "Marquer lu / non lu",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_marquer(&self, Parameters(p): Parameters<MarquerParams>) -> Sortie<Fait> {
        let mut corps = ids_json(&p.ids)?;
        corps["unread"] = Value::Bool(matches!(p.etat, Lecture::NonLu));
        self.ent
            .ecrire_sans_retour(
                Method::POST,
                "/conversation/toggleUnread",
                Corps::Json(&corps),
            )
            .await?;
        Ok(Json(Fait {
            fait: true,
            nombre: p.ids.len(),
        }))
    }

    #[tool(
        description = "Supprime des messages : vers la corbeille (défaut, récupérable) ou définitivement \
                       (`mode: definitive`, uniquement pour des messages déjà dans la corbeille).",
        annotations(
            title = "Supprimer des messages",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_supprimer(
        &self,
        Parameters(p): Parameters<SupprimerParams>,
    ) -> Sortie<Fait> {
        let corps = ids_json(&p.ids)?;
        let route = match p.mode {
            Suppression::Corbeille => "/conversation/trash",
            Suppression::Definitive => "/conversation/delete",
        };
        self.ent
            .ecrire_sans_retour(Method::PUT, route, Corps::Json(&corps))
            .await?;
        Ok(Json(Fait {
            fait: true,
            nombre: p.ids.len(),
        }))
    }

    #[tool(
        description = "Restaure des messages depuis la corbeille.",
        annotations(
            title = "Restaurer des messages",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_restaurer(&self, Parameters(p): Parameters<IdsParams>) -> Sortie<Fait> {
        let corps = ids_json(&p.ids)?;
        self.ent
            .ecrire_sans_retour(Method::PUT, "/conversation/restore", Corps::Json(&corps))
            .await?;
        Ok(Json(Fait {
            fait: true,
            nombre: p.ids.len(),
        }))
    }

    #[tool(
        description = "Rappelle (annule) un message envoyé il y a moins de 60 minutes.",
        annotations(
            title = "Rappeler un message",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn messagerie_rappeler(&self, Parameters(p): Parameters<IdParams>) -> Sortie<Fait> {
        let c = chemin(&["/conversation/api/messages/", p.id.as_str(), "/recall"])?;
        self.ent
            .ecrire_sans_retour::<()>(Method::POST, &c, Corps::Aucun)
            .await?;
        Ok(Json(Fait {
            fait: true,
            nombre: 1,
        }))
    }

    #[tool(
        description = "Arborescence des dossiers personnels de la messagerie (identifiants, noms, nombres de non lus).",
        annotations(
            title = "Dossiers de la messagerie",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_dossiers(&self) -> Sortie<Dossiers> {
        let dossiers: Value = self
            .ent
            .get_avec("/conversation/api/folders", Some(&[("depth", "3")]))
            .await?;
        Ok(Json(Dossiers { dossiers }))
    }

    #[tool(
        description = "Crée un dossier personnel dans la messagerie.",
        annotations(
            title = "Créer un dossier",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn messagerie_creer_dossier(
        &self,
        Parameters(p): Parameters<CreerDossierParams>,
    ) -> Sortie<DossierCree> {
        let mut corps = json!({ "name": p.nom });
        if let Some(parent) = p.parent_id {
            corps["parentId"] = Value::String(parent);
        }
        let v: Value = self
            .ent
            .ecrire(Method::POST, "/conversation/folder", Corps::Json(&corps))
            .await?;
        Ok(Json(DossierCree {
            id: v.get("id").cloned().unwrap_or(v),
        }))
    }

    #[tool(
        description = "Déplace des messages vers la boîte de réception ou un dossier personnel.",
        annotations(
            title = "Déplacer des messages",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_deplacer(&self, Parameters(p): Parameters<DeplacerParams>) -> Sortie<Fait> {
        let corps = ids_json(&p.ids)?;
        match &p.vers {
            Dossier::Reception => {
                // L'ENT attend ici les identifiants en paramètres d'URL.
                let mut c = String::from("/conversation/move/root?");
                for (i, id) in p.ids.iter().enumerate() {
                    if i > 0 {
                        c.push('&');
                    }
                    c.push_str(&chemin(&["id=", id.as_str()])?);
                }
                self.ent
                    .ecrire_sans_retour::<()>(Method::PUT, &c, Corps::Aucun)
                    .await?;
            }
            Dossier::Corbeille => {
                self.ent
                    .ecrire_sans_retour(Method::PUT, "/conversation/trash", Corps::Json(&corps))
                    .await?;
            }
            Dossier::Personnel(_) => {
                let c = chemin(&["/conversation/move/userfolder/", p.vers.code()?])?;
                self.ent
                    .ecrire_sans_retour(Method::PUT, &c, Corps::Json(&corps))
                    .await?;
            }
            Dossier::Envoyes | Dossier::Brouillons => {
                return Err(
                    "on ne peut déplacer que vers `reception`, `corbeille` ou un dossier personnel"
                        .into(),
                );
            }
        }
        Ok(Json(Fait {
            fait: true,
            nombre: p.ids.len(),
        }))
    }

    #[tool(
        description = "Télécharge une pièce jointe d'un message (contenu en base64, 10 Mo maximum).",
        annotations(
            title = "Télécharger une pièce jointe",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn messagerie_telecharger_piece_jointe(
        &self,
        Parameters(p): Parameters<PieceJointeParams>,
    ) -> Sortie<Fichier> {
        let c = chemin(&[
            "/conversation/message/",
            p.message_id.as_str(),
            "/attachment/",
            &p.piece_jointe_id,
        ])?;
        let (octets, mime) = self.ent.telecharger(&c).await?;
        Ok(Json(fichier_sortant(p.piece_jointe_id, &octets, mime)?))
    }

    #[tool(
        description = "Joint un fichier (base64, 10 Mo maximum) à un brouillon.",
        annotations(
            title = "Joindre un fichier",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn messagerie_joindre_fichier(
        &self,
        Parameters(p): Parameters<JoindreParams>,
    ) -> Sortie<PieceJointeAjoutee> {
        let octets = fichier_entrant(&p.contenu_base64)?;
        let c = chemin(&[
            "/conversation/message/",
            p.brouillon_id.as_str(),
            "/attachment",
        ])?;
        let v: Value = self.ent.televerser(&c, &p.nom_fichier, octets, &[]).await?;
        Ok(Json(PieceJointeAjoutee {
            id: v.get("id").cloned().unwrap_or(v),
        }))
    }
}
