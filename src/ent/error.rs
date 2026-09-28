use reqwest::StatusCode;

/// Erreurs remontées par le client ENT.
///
/// Les messages sont rédigés pour être lus tels quels par l'assistant
/// (et donc, indirectement, par le parent) : ils doivent dire quoi faire.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EntError {
    #[error("l'ENT a refusé l'identifiant ou le mot de passe")]
    IdentifiantsRefuses,

    #[error(
        "l'ENT demande une action dans le navigateur avant toute connexion automatique \
         (redirection vers {0}) : connectez-vous une fois sur le site, puis réessayez"
    )]
    ActionRequise(Box<str>),

    #[error("la session ENT a expiré et la reconnexion automatique a échoué")]
    SessionExpiree,

    #[error("l'ENT a répondu {status} sur {chemin}")]
    Http {
        status: StatusCode,
        chemin: Box<str>,
    },

    #[error("erreur réseau en joignant l'ENT : {0}")]
    Reseau(#[from] reqwest::Error),

    #[error("réponse inattendue de l'ENT sur {chemin} : {source}")]
    Decodage {
        chemin: Box<str>,
        source: serde_json::Error,
    },

    #[error("{0}")]
    Invalide(Box<str>),
}

pub type Result<T, E = EntError> = std::result::Result<T, E>;

/// Les outils MCP renvoient leurs erreurs sous forme de texte lisible.
impl From<EntError> for String {
    fn from(e: EntError) -> Self {
        e.to_string()
    }
}
