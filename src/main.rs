//! Serveur MCP pour l'ENT ONE (plateforme Edifice).
//!
//! Deux transports :
//! - `stdio` : lancé par un client local (Claude Desktop, Claude Code…) ;
//! - `http`  : Streamable HTTP, pour un assistant dans le cloud, protégé
//!   par un jeton porteur.

#![forbid(unsafe_code)]

mod ent;
mod outils;
mod texte;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, bail};
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use clap::{Parser, ValueEnum};
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use crate::ent::{EntClient, Identifiants};
use crate::outils::Ent1Server;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Transport {
    /// Entrée/sortie standard (client MCP local).
    Stdio,
    /// Streamable HTTP (assistant distant).
    Http,
}

/// Serveur MCP pour l'ENT ONE (Edifice) : messagerie, carnet de liaison,
/// cahier de texte, actualités, blogs, calendrier, documents, casier…
#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = AIDE_SECRETS)]
struct Options {
    /// Adresse de l'ENT.
    #[arg(long, env = "ENT1_URL", default_value = ent::client::URL_PAR_DEFAUT)]
    url: reqwest::Url,

    /// Identifiant ENT (le même que sur la page de connexion). Sans identifiant,
    /// le serveur démarre quand même et chaque outil explique comment le configurer.
    #[arg(long, env = "ENT1_IDENTIFIANT")]
    identifiant: Option<String>,

    /// Fuseau horaire des dates renvoyées.
    #[arg(long, env = "ENT1_FUSEAU", default_value = "Indian/Reunion")]
    fuseau: String,

    #[arg(long, env = "ENT1_TRANSPORT", value_enum, default_value_t = Transport::Stdio)]
    transport: Transport,

    /// Adresse d'écoute en mode HTTP.
    #[arg(long, env = "ENT1_ECOUTE", default_value = "127.0.0.1:8080")]
    ecoute: SocketAddr,

    /// Noms d'hôte publics acceptés en mode HTTP (protection DNS rebinding),
    /// séparés par des virgules. Par défaut : boucle locale uniquement.
    #[arg(long, env = "ENT1_HOTES", value_delimiter = ',')]
    hotes: Vec<String>,
}

/// Les secrets ne passent jamais en argument : ils apparaîtraient dans la
/// liste des processus.
const AIDE_SECRETS: &str = "Secrets (variables d'environnement uniquement) :
  ENT1_MOT_DE_PASSE  mot de passe ENT
  ENT1_MCP_JETON     jeton d'accès exigé en mode HTTP (32 caractères minimum)";

/// Lit un secret dans l'environnement.
fn secret(nom: &str) -> Option<SecretString> {
    std::env::var(nom)
        .ok()
        .filter(|v| !v.is_empty())
        .map(SecretString::from)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Journaux sur stderr : stdout appartient au protocole en mode stdio.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("ent1_mcp=info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let opts = Options::parse();
    let fuseau = jiff::tz::TimeZone::get(&opts.fuseau)
        .with_context(|| format!("fuseau horaire inconnu : {}", opts.fuseau))?;

    let identifiants = match (
        opts.identifiant
            .as_deref()
            .map(str::trim)
            .filter(|i| !i.is_empty()),
        secret("ENT1_MOT_DE_PASSE"),
    ) {
        (Some(identifiant), Some(mot_de_passe)) => Some(Identifiants {
            identifiant: identifiant.into(),
            mot_de_passe,
        }),
        (None, None) => {
            tracing::warn!(
                "ENT1_IDENTIFIANT et ENT1_MOT_DE_PASSE absents : démarrage sans compte, \
                 les outils répondront « non configuré »"
            );
            None
        }
        (Some(_), None) => bail!("ENT1_IDENTIFIANT est défini mais ENT1_MOT_DE_PASSE est absent"),
        (None, Some(_)) => bail!("ENT1_MOT_DE_PASSE est défini mais ENT1_IDENTIFIANT est absent"),
    };
    let configure = identifiants.is_some();

    let ent = Arc::new(EntClient::new(opts.url.clone(), identifiants)?);
    if configure {
        // Échouer tôt : des identifiants faux se voient au démarrage, pas au premier outil.
        ent.connecter()
            .await
            .with_context(|| format!("connexion à {} impossible", opts.url))?;
    }

    match opts.transport {
        Transport::Stdio => {
            let service = Ent1Server::new(ent, fuseau)
                .serve(rmcp::transport::stdio())
                .await
                .context("démarrage du transport stdio")?;
            service.waiting().await?;
        }
        Transport::Http => {
            let Some(jeton) = secret("ENT1_MCP_JETON") else {
                bail!("le mode HTTP exige ENT1_MCP_JETON (32 caractères minimum)");
            };
            if jeton.expose_secret().len() < 32 {
                bail!("ENT1_MCP_JETON est trop court (32 caractères minimum)");
            }
            servir_http(ent, fuseau, opts.ecoute, jeton, opts.hotes).await?;
        }
    }
    Ok(())
}

async fn servir_http(
    ent: Arc<EntClient>,
    fuseau: jiff::tz::TimeZone,
    ecoute: SocketAddr,
    jeton: SecretString,
    hotes: Vec<String>,
) -> anyhow::Result<()> {
    let arret = CancellationToken::new();
    let mut config = StreamableHttpServerConfig::default();
    config.cancellation_token = arret.child_token();
    if !hotes.is_empty() {
        config.allowed_hosts.extend(hotes);
    }

    let service: StreamableHttpService<Ent1Server, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(Ent1Server::new(Arc::clone(&ent), fuseau.clone())),
            Arc::default(),
            config,
        );

    let app = axum::Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(
            Arc::new(jeton),
            verifier_jeton,
        ))
        .route("/sante", axum::routing::get(|| async { "ok" }));

    let ecouteur = tokio::net::TcpListener::bind(ecoute)
        .await
        .with_context(|| format!("écoute sur {ecoute} impossible"))?;
    tracing::info!("serveur MCP en écoute sur http://{ecoute}/mcp");

    axum::serve(ecouteur, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            arret.cancel();
        })
        .await?;
    Ok(())
}

async fn verifier_jeton(
    State(jeton): State<Arc<SecretString>>,
    req: Request,
    suite: Next,
) -> Response {
    let fourni = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    let attendu = jeton.expose_secret().as_bytes();
    if bool::from(fourni.as_bytes().ct_eq(attendu)) {
        suite.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "jeton manquant ou invalide",
        )
            .into_response()
    }
}
