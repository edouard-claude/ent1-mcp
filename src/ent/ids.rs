//! Identifiants typés : impossible de passer un identifiant de message
//! là où l'ENT attend un identifiant d'utilisateur.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! id_texte {
    ($(#[$doc:meta])* $nom:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $nom(Box<str>);

        impl $nom {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $nom {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $nom {
            fn from(s: &str) -> Self {
                Self(s.into())
            }
        }
    };
}

id_texte!(
    /// Identifiant d'un message de la messagerie.
    MessageId
);
id_texte!(
    /// Identifiant d'un utilisateur ou d'un groupe (destinataire).
    DestinataireId
);
id_texte!(
    /// Identifiant d'un utilisateur de l'ENT (parent, élève, enseignant).
    UtilisateurId
);
id_texte!(
    /// Identifiant d'un cahier de texte.
    CahierId
);

/// Identifiant d'un mot du carnet de liaison (numérique côté ENT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct MotId(pub u64);

impl fmt::Display for MotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Un identifiant ne doit jamais pouvoir modifier le chemin d'une URL.
pub(crate) fn segment_sur(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::segment_sur;

    #[test]
    fn refuse_les_segments_dangereux() {
        assert!(segment_sur("00000000-0000-4000-8000-000000000000"));
        assert!(!segment_sur(""));
        assert!(!segment_sur("../auth/logout"));
        assert!(!segment_sur("abc?x=1"));
        assert!(!segment_sur("abc/def"));
    }
}
