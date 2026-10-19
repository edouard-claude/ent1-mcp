//! Outils MCP exposés aux assistants, regroupés par application de l'ENT.
//!
//! Chaque module ajoute un routeur d'outils à [`Ent1Server`] ; ils sont
//! assemblés dans [`Ent1Server::new`].

mod actualites;
mod annuaire;
mod blog;
mod cahier;
mod calendrier;
mod carnet;
mod casier;
mod documents;
mod fil;
mod formulaires;
mod messagerie;
mod profil;
mod wiki;

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use jiff::tz::TimeZone;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{Json, ServerHandler, tool_handler};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::ent::EntClient;

/// Taille maximale d'un fichier échangé en base64 (entrée ou sortie).
const TAILLE_MAX_FICHIER: usize = 10 * 1024 * 1024;

/// Résultat d'un outil : sortie structurée, ou message d'erreur lisible
/// renvoyé à l'assistant avec `isError: true`.
type Sortie<T> = Result<Json<T>, String>;

#[derive(Debug, Clone)]
pub struct Ent1Server {
    /// Partagé par toutes les sessions MCP (une instance par session en
    /// HTTP) : un seul compte ENT, une seule session de cookies.
    ent: Arc<EntClient>,
    fuseau: TimeZone,
    tool_router: ToolRouter<Self>,
}

impl Ent1Server {
    pub fn new(ent: Arc<EntClient>, fuseau: TimeZone) -> Self {
        Self {
            ent,
            fuseau,
            tool_router: Self::profil_router()
                + Self::fil_router()
                + Self::messagerie_router()
                + Self::annuaire_router()
                + Self::carnet_router()
                + Self::cahier_router()
                + Self::actualites_router()
                + Self::blog_router()
                + Self::calendrier_router()
                + Self::documents_router()
                + Self::casier_router()
                + Self::formulaires_router()
                + Self::wiki_router(),
        }
    }

    fn locale(&self, ts: jiff::Timestamp) -> String {
        ts.to_zoned(self.fuseau.clone())
            .strftime("%Y-%m-%dT%H:%M:%S%:z")
            .to_string()
    }

    /// Horodatage en millisecondes → date locale lisible (RFC 3339).
    fn date_ms(&self, ms: i64) -> Option<String> {
        jiff::Timestamp::from_millisecond(ms)
            .ok()
            .map(|ts| self.locale(ts))
    }

    /// Date textuelle : convertie en heure locale si elle porte un fuseau,
    /// laissée telle quelle sinon (l'ENT stocke parfois des heures locales).
    fn date_texte(&self, s: &str) -> String {
        s.parse::<jiff::Timestamp>()
            .map_or_else(|_| s.to_owned(), |ts| self.locale(ts))
    }

    /// Date au format MongoDB (`{"$date": ...}`), chaîne ISO ou nombre.
    fn date_valeur(&self, v: &Value) -> Option<String> {
        match v {
            Value::Object(o) => o.get("$date").and_then(|d| self.date_valeur(d)),
            Value::Number(n) => n.as_i64().and_then(|ms| self.date_ms(ms)),
            Value::String(s) => Some(self.date_texte(s)),
            _ => None,
        }
    }

    fn lien(&self, chemin: &str) -> String {
        self.ent
            .base()
            .join(chemin)
            .map_or_else(|_| chemin.to_owned(), |u| u.to_string())
    }
}

#[allow(clippy::unused_async_trait_impl)] // `get_info` et consorts sont générés par la macro
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Ent1Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("ent1-mcp", env!("CARGO_PKG_VERSION"))
                    .with_title("ENT ONE (Edifice)")
                    .with_website_url("https://github.com/edouard-claude/ent1-mcp"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}

const INSTRUCTIONS: &str = "\
Accès à l'ENT ONE (Edifice) d'un parent ou d'un personnel : messagerie, carnet \
de liaison, cahier de texte, actualités, blogs, calendrier, espace documentaire, \
casier, formulaires, wiki, annuaire et fil de nouveautés.

Bonnes pratiques :
- Commencer par `profil` pour connaître le type de compte et les enfants (leurs identifiants servent au carnet de liaison).
- Pour écrire à un enseignant : `annuaire_rechercher` pour obtenir son identifiant, puis `messagerie_envoyer`.
- Avant tout envoi (message, réponse au carnet, commentaire, dépôt de fichier), montrer le contenu exact à l'utilisateur et attendre son accord : un message envoyé ne peut être rappelé que pendant 60 minutes.
- Les contenus sont en texte brut : l'ENT met en forme les paragraphes.
- Les dates sont dans le fuseau horaire configuré (par défaut Indian/Reunion).";

/// Fichier transmis à l'assistant.
#[derive(Debug, Serialize, JsonSchema)]
struct Fichier {
    nom: String,
    type_mime: Option<String>,
    taille_octets: usize,
    /// Contenu encodé en base64 (standard, avec remplissage).
    contenu_base64: String,
}

fn fichier_sortant(
    nom: String,
    octets: &[u8],
    type_mime: Option<String>,
) -> Result<Fichier, String> {
    if octets.len() > TAILLE_MAX_FICHIER {
        return Err(format!(
            "fichier trop volumineux ({} octets, maximum {TAILLE_MAX_FICHIER}) : téléchargez-le depuis l'ENT",
            octets.len()
        ));
    }
    Ok(Fichier {
        nom,
        type_mime,
        taille_octets: octets.len(),
        contenu_base64: BASE64.encode(octets),
    })
}

fn fichier_entrant(contenu_base64: &str) -> Result<Vec<u8>, String> {
    // Borne avant décodage : 4 caractères base64 = 3 octets.
    if contenu_base64.len() / 4 * 3 > TAILLE_MAX_FICHIER {
        return Err(format!(
            "fichier trop volumineux (maximum {TAILLE_MAX_FICHIER} octets)"
        ));
    }
    let nettoye: String = contenu_base64
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    BASE64
        .decode(nettoye.as_bytes())
        .map_err(|e| format!("contenu_base64 invalide : {e}"))
}

fn page_defaut() -> u32 {
    0
}

/// Première chaîne trouvée parmi `cles` (les routes peu documentées de
/// l'ENT nomment les mêmes champs différemment selon les applications).
fn champ(v: &Value, cles: &[&str]) -> Option<String> {
    cles.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .map(str::to_owned)
}

/// Identifiant (`_id` ou `id`, texte ou nombre) sous forme de texte.
fn identifiant(v: &Value) -> String {
    match v.get("_id").or_else(|| v.get("id")) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)] // tests
mod tests {
    use super::*;

    #[test]
    fn aller_retour_base64() {
        let f = fichier_sortant("a.txt".into(), b"bonjour", None);
        let Ok(f) = f else {
            panic!("fichier_sortant a échoué")
        };
        assert_eq!(
            fichier_entrant(&f.contenu_base64).ok().as_deref(),
            Some(&b"bonjour"[..])
        );
    }

    #[test]
    fn catalogue_des_outils() {
        let base = reqwest::Url::parse("https://ent.invalid").expect("URL de test valide");
        let ent = EntClient::new(
            base,
            Some(crate::ent::Identifiants {
                identifiant: "test".into(),
                mot_de_passe: "test".to_owned().into(),
            }),
        )
        .expect("client de test");
        let serveur = Ent1Server::new(Arc::new(ent), TimeZone::UTC);
        let outils = serveur.tool_router.list_all();
        let noms: Vec<&str> = outils.iter().map(|t| t.name.as_ref()).collect();
        for attendu in [
            "profil",
            "fil_nouveautes",
            "messagerie_envoyer",
            "carnet_signer",
            "cahier_lire",
            "casier_deposer",
        ] {
            assert!(noms.contains(&attendu), "outil manquant : {attendu}");
        }
        for outil in &outils {
            assert!(
                outil.description.as_ref().is_some_and(|d| !d.is_empty()),
                "{} sans description",
                outil.name
            );
            assert!(
                outil.annotations.is_some(),
                "{} sans annotations",
                outil.name
            );
            let schema = serde_json::to_value(&outil.input_schema).expect("schéma sérialisable");
            assert_eq!(
                schema["type"], "object",
                "{} : schéma d'entrée non objet",
                outil.name
            );
        }
        println!("{} outils : {}", outils.len(), noms.join(", "));
    }

    #[test]
    fn refuse_base64_invalide() {
        assert!(fichier_entrant("@@@").is_err());
    }
}
