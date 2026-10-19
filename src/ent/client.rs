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
        if !session_perdue(&rep, &self.base) {
            return verifier(rep, chemin);
        }
        tracing::debug!(chemin, "session perdue, reconnexion");
        self.reconnecter(generation).await?;
        let rep = construire()?.send().await?;
        if session_perdue(&rep, &self.base) {
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

    /// Ferme la session côté ENT, puis en rouvre une aussitôt : l'outil `deconnexion` sert
    /// à repartir d'une session propre, pas à rester déconnecté.
    pub async fn deconnecter(&self) -> Result<()> {
        {
            let _garde = self.connexion.lock().await;
            let _ = self.http.get(self.url("/auth/logout")?).send().await?;
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
        self.connecter().await
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

/// 401, ou redirection qui ne sert pas de contenu : vers l'ENT lui-même (une API ne
/// redirige jamais vers une autre de ses pages quand la session tient), ou vers une page
/// d'authentification ailleurs. Seule une redirection vers un autre hôte qui n'est pas une
/// page de connexion (téléchargement servi ailleurs) garde la session.
///
/// Avant, seule une redirection vers `/auth/login` comptait : l'ENT renvoie aussi des 302
/// vers d'autres adresses quand la session meurt, et le serveur ne se reconnectait plus
/// (relevé des devoirs du 19/10 : « 302 Found sur /homeworks/get/… »).
fn session_perdue(rep: &Response, base: &Url) -> bool {
    let cible = rep.headers().get(LOCATION).and_then(|v| v.to_str().ok());
    perte_de_session(rep.status(), cible, base)
}

fn perte_de_session(status: StatusCode, cible: Option<&str>, base: &Url) -> bool {
    if status == StatusCode::UNAUTHORIZED {
        return true;
    }
    if !status.is_redirection() {
        return false;
    }
    let Some(url) = cible.and_then(|c| base.join(c).ok()) else {
        return true;
    };
    let chez_nous = url.host_str() == base.host_str();
    let authentification = ["/auth", "login", "/cas", "/sso", "oauth", "saml"]
        .iter()
        .any(|m| url.path().to_lowercase().contains(m));
    chez_nous || authentification
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
    fn une_redirection_vers_l_ent_est_une_session_perdue()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let base = Url::parse("https://ent.exemple.fr/")?;
        let perdue = |c: Option<&str>| perte_de_session(StatusCode::FOUND, c, &base);
        assert!(perdue(Some("/auth/login?callback=%2Fhomeworks")));
        assert!(perdue(Some("/timeline/timeline")), "toute page de l'ENT");
        assert!(perdue(Some("https://ent.exemple.fr/auth/oauth2/userinfo")));
        assert!(
            perdue(Some("https://sso.academie.fr/cas/login")),
            "SSO ailleurs"
        );
        assert!(perdue(None), "302 sans adresse");
        assert!(
            !perdue(Some("https://stockage.exemple.net/fichiers/abc.pdf")),
            "téléchargement servi ailleurs"
        );
        assert!(perte_de_session(StatusCode::UNAUTHORIZED, None, &base));
        assert!(!perte_de_session(StatusCode::OK, None, &base));
        Ok(())
    }

    #[test]
    fn detecte_les_actions_requises() {
        assert!(exige_une_action("/auth/revalidate-terms"));
        assert!(exige_une_action("/auth/validate-mfa"));
        assert!(!exige_une_action("/timeline/timeline"));
    }
}
