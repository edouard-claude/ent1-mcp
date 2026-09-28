//! Client HTTP de l'ENT : connexion par formulaire, cookie de session,
//! jeton anti-CSRF et reconnexion transparente quand la session expire.
//!
//! Propriété : un `EntClient` par compte ENT. Il est partagé (via `Arc`)
//! par toutes les instances du serveur MCP, qui ne font que le lire ;
//! seul le magasin de cookies (interne à reqwest) et le verrou de
//! connexion portent de l'état mutable.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{ACCEPT, HeaderValue, LOCATION};
use reqwest::{Method, RequestBuilder, Response, StatusCode, Url, multipart, redirect};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use super::error::{EntError, Result};
use super::ids::segment_sur;

/// Instance ENT ONE de l'académie de La Réunion (1er degré).
pub const URL_PAR_DEFAUT: &str = "https://ent1d.ac-reunion.fr";

const EN_TETE_XSRF: &str = "X-XSRF-TOKEN";
const COOKIE_XSRF: &str = "XSRF-TOKEN";

#[derive(Debug)]
pub struct Identifiants {
    pub identifiant: Box<str>,
    pub mot_de_passe: SecretString,
}

/// Corps d'une requête sortante.
#[derive(Debug)]
pub enum Corps<'a, B: Serialize + ?Sized = ()> {
    Aucun,
    Json(&'a B),
}

#[derive(Debug)]
pub struct EntClient {
    http: reqwest::Client,
    /// Partagé avec `http` : on y lit le cookie XSRF-TOKEN.
    cookies: Arc<Jar>,
    base: Url,
    identifiants: Identifiants,
    /// Tenu à travers `.await` volontairement : il sérialise les
    /// reconnexions, pour que N requêtes en 401 simultanées ne déclenchent
    /// qu'une seule connexion (et pas N tentatives de mot de passe).
    connexion: Mutex<()>,
    /// Incrémenté à chaque connexion réussie.
    generation: AtomicU64,
    /// Levé après un refus des identifiants : on ne réessaie plus jamais
    /// automatiquement, pour ne pas faire verrouiller le compte.
    identifiants_refuses: AtomicBool,
}

impl EntClient {
    pub fn new(base: Url, identifiants: Identifiants) -> Result<Self> {
        let cookies = Arc::new(Jar::default());
        let http = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&cookies))
            // Une redirection vers /auth/login signale une session perdue :
            // on veut la voir, pas la suivre.
            .redirect(redirect::Policy::none())
            .user_agent(concat!("ent1-mcp/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            http,
            cookies,
            base,
            identifiants,
            connexion: Mutex::new(()),
            generation: AtomicU64::new(0),
            identifiants_refuses: AtomicBool::new(false),
        })
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    /// Ouvre (ou rouvre) une session. Appelé au démarrage pour échouer tôt,
    /// puis automatiquement quand l'ENT signale une session expirée.
    pub async fn connecter(&self) -> Result<()> {
        let generation = self.generation.load(Ordering::Acquire);
        self.reconnecter(generation).await
    }

    async fn reconnecter(&self, generation_vue: u64) -> Result<()> {
        let _garde = self.connexion.lock().await;
        if self.generation.load(Ordering::Acquire) != generation_vue {
            return Ok(()); // Une autre tâche vient de se reconnecter.
        }
        if self.identifiants_refuses.load(Ordering::Acquire) {
            return Err(EntError::IdentifiantsRefuses);
        }

        let formulaire = [
            ("email", self.identifiants.identifiant.as_ref()),
            ("password", self.identifiants.mot_de_passe.expose_secret()),
            ("callBack", ""),
            ("details", ""),
        ];
        let rep = self
            .http
            .post(self.url("/auth/login")?)
            .form(&formulaire)
            .send()
            .await?;

        match rep.status() {
            s if s.is_redirection() => {
                let cible = rep
                    .headers()
                    .get(LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default();
                if cible.contains("/auth/login") {
                    self.identifiants_refuses.store(true, Ordering::Release);
                    return Err(EntError::IdentifiantsRefuses);
                }
                if exige_une_action(cible) {
                    return Err(EntError::ActionRequise(cible.into()));
                }
            }
            // L'ENT renvoie la page de connexion (200) avec un message d'erreur.
            StatusCode::OK => {
                self.identifiants_refuses.store(true, Ordering::Release);
                return Err(EntError::IdentifiantsRefuses);
            }
            status => {
                return Err(EntError::Http {
                    status,
                    chemin: "/auth/login".into(),
                });
            }
        }

        if self.xsrf().is_none() {
            // Le jeton anti-CSRF est posé au premier rendu d'une page.
            let _ = self
                .http
                .get(self.url("/timeline/timeline")?)
                .send()
                .await?;
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
        tracing::info!("session ENT ouverte");
        Ok(())
    }

    fn url(&self, chemin: &str) -> Result<Url> {
        self.base
            .join(chemin)
            .map_err(|e| EntError::Invalide(format!("chemin invalide {chemin} : {e}").into()))
    }

    fn xsrf(&self) -> Option<HeaderValue> {
        let cookies = self.cookies.cookies(&self.base)?;
        let brut = cookies.to_str().ok()?;
        brut.split(';')
            .filter_map(|c| c.trim().split_once('='))
            .find(|(nom, _)| *nom == COOKIE_XSRF)
            .and_then(|(_, v)| HeaderValue::from_str(v).ok())
    }

    fn preparer(&self, methode: &Method, chemin: &str) -> Result<RequestBuilder> {
        let mut req = self
            .http
            .request(methode.clone(), self.url(chemin)?)
            .header(ACCEPT, "application/json");
        if *methode != Method::GET
            && let Some(jeton) = self.xsrf()
        {
            req = req.header(EN_TETE_XSRF, jeton);
        }
        Ok(req)
    }

    /// Envoie une requête ; en cas de session perdue, se reconnecte une
    /// fois et rejoue la requête.
    async fn executer<F>(&self, chemin: &str, construire: F) -> Result<Response>
    where
        F: Fn() -> Result<RequestBuilder>,
    {
        let generation = self.generation.load(Ordering::Acquire);
        let rep = construire()?.send().await?;
        if !session_perdue(&rep) {
            return verifier(rep, chemin);
        }
        tracing::debug!(chemin, "session perdue, reconnexion");
        self.reconnecter(generation).await?;
        let rep = construire()?.send().await?;
        if session_perdue(&rep) {
            return Err(EntError::SessionExpiree);
        }
        verifier(rep, chemin)
    }

    pub async fn requete<B, Q>(
        &self,
        methode: Method,
        chemin: &str,
        query: Option<&Q>,
        corps: Corps<'_, B>,
    ) -> Result<Response>
    where
        B: Serialize + ?Sized,
        Q: Serialize + ?Sized,
    {
        self.executer(chemin, || {
            let mut req = self.preparer(&methode, chemin)?;
            if let Some(q) = query {
                req = req.query(q);
            }
            if let Corps::Json(b) = &corps {
                req = req.json(b);
            }
            Ok(req)
        })
        .await
    }

    /// GET décodé en `T`.
    pub async fn get<T: DeserializeOwned>(&self, chemin: &str) -> Result<T> {
        self.get_avec::<T, ()>(chemin, None).await
    }

    pub async fn get_avec<T, Q>(&self, chemin: &str, query: Option<&Q>) -> Result<T>
    where
        T: DeserializeOwned,
        Q: Serialize + ?Sized,
    {
        let rep = self
            .requete::<(), Q>(Method::GET, chemin, query, Corps::Aucun)
            .await?;
        decoder(rep, chemin).await
    }

    /// Requête d'écriture dont la réponse est décodée en `T`.
    pub async fn ecrire<T, B>(
        &self,
        methode: Method,
        chemin: &str,
        corps: Corps<'_, B>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let rep = self.requete::<B, ()>(methode, chemin, None, corps).await?;
        decoder(rep, chemin).await
    }

    /// Requête d'écriture dont la réponse est ignorée.
    pub async fn ecrire_sans_retour<B>(
        &self,
        methode: Method,
        chemin: &str,
        corps: Corps<'_, B>,
    ) -> Result<()>
    where
        B: Serialize + ?Sized,
    {
        self.requete::<B, ()>(methode, chemin, None, corps).await?;
        Ok(())
    }

    /// Téléverse un fichier en `multipart/form-data` (champ `file`).
    pub async fn televerser<T: DeserializeOwned>(
        &self,
        chemin: &str,
        nom_fichier: &str,
        contenu: Vec<u8>,
        champs: &[(&str, &str)],
    ) -> Result<T> {
        // `Bytes` : le rejeu après reconnexion partage le tampon au lieu de le copier.
        let contenu = bytes::Bytes::from(contenu);
        let rep = self
            .executer(chemin, || {
                let mut form = multipart::Form::new().part(
                    "file",
                    multipart::Part::stream_with_length(
                        reqwest::Body::from(contenu.clone()),
                        contenu.len() as u64,
                    )
                    .file_name(nom_fichier.to_owned()),
                );
                for (k, v) in champs {
                    form = form.text((*k).to_owned(), (*v).to_owned());
                }
                Ok(self.preparer(&Method::POST, chemin)?.multipart(form))
            })
            .await?;
        decoder(rep, chemin).await
    }

    /// Télécharge un contenu binaire (pièce jointe, document).
    pub async fn telecharger(&self, chemin: &str) -> Result<(Vec<u8>, Option<String>)> {
        let rep = self
            .requete::<(), ()>(Method::GET, chemin, None, Corps::Aucun)
            .await?;
        let type_mime = rep
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        Ok((rep.bytes().await?.to_vec(), type_mime))
    }

    /// Ferme la session côté ENT.
    pub async fn deconnecter(&self) -> Result<()> {
        let _garde = self.connexion.lock().await;
        let _ = self.http.get(self.url("/auth/logout")?).send().await?;
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

/// Construit un chemin `/prefixe/{id}/...` en refusant tout identifiant
/// capable de sortir du segment (`/`, `..`, `?`).
pub fn chemin(morceaux: &[&str]) -> Result<String> {
    let mut sortie = String::new();
    for (i, m) in morceaux.iter().enumerate() {
        // Les morceaux pairs sont littéraux (écrits dans le code),
        // les impairs viennent de l'appelant et sont vérifiés.
        if i % 2 == 1 && !segment_sur(m) {
            return Err(EntError::Invalide(
                format!("identifiant invalide : {m:?}").into(),
            ));
        }
        sortie.push_str(m);
    }
    Ok(sortie)
}

/// 401, ou redirection vers la page de connexion. Les autres redirections
/// (téléchargement servi ailleurs, par exemple) ne sont pas des pertes de session.
fn session_perdue(rep: &Response) -> bool {
    match rep.status() {
        StatusCode::UNAUTHORIZED => true,
        s if s.is_redirection() => rep
            .headers()
            .get(LOCATION)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|cible| cible.contains("/auth/login")),
        _ => false,
    }
}

fn exige_une_action(cible: &str) -> bool {
    [
        "revalidate",
        "cgu",
        "mfa",
        "otp",
        "activation",
        "reset",
        "changePassword",
        "validate",
    ]
    .iter()
    .any(|m| cible.contains(m))
}

fn verifier(rep: Response, chemin: &str) -> Result<Response> {
    let status = rep.status();
    if status.is_success() {
        Ok(rep)
    } else {
        Err(EntError::Http {
            status,
            chemin: chemin.into(),
        })
    }
}

async fn decoder<T: DeserializeOwned>(rep: Response, chemin: &str) -> Result<T> {
    let octets = rep.bytes().await?;
    // Certaines routes d'écriture répondent un corps vide.
    let octets: &[u8] = if octets.iter().all(u8::is_ascii_whitespace) {
        b"null"
    } else {
        &octets
    };
    serde_json::from_slice(octets).map_err(|source| EntError::Decodage {
        chemin: chemin.into(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chemin_verifie_les_identifiants() {
        assert_eq!(
            chemin(&["/conversation/api/messages/", "abc-123"])
                .ok()
                .as_deref(),
            Some("/conversation/api/messages/abc-123")
        );
        assert!(chemin(&["/conversation/api/messages/", "../../auth/logout"]).is_err());
    }

    #[test]
    fn detecte_les_actions_requises() {
        assert!(exige_une_action("/auth/revalidate-terms"));
        assert!(exige_une_action("/auth/validate-mfa"));
        assert!(!exige_une_action("/timeline/timeline"));
    }
}
