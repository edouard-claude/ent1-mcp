use reqwest::Method;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Ent1Server, Sortie, champ, identifiant, page_defaut};
use crate::ent::{Corps, chemin};
use crate::texte::html_vers_texte;

#[derive(Debug, Serialize, JsonSchema)]
struct Blogs {
    blogs: Vec<BlogSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct BlogSortie {
    id: String,
    titre: String,
    description: String,
    auteur: Option<String>,
    modifie_le: Option<String>,
    /// `IMMEDIATE` (commentaires publiés directement), `RESTRAINT` (modérés) ou `NONE`.
    commentaires: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BilletsParams {
    blog_id: String,
    #[serde(default = "page_defaut")]
    page: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Billets {
    billets: Vec<BilletSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct BilletSortie {
    id: String,
    titre: String,
    contenu: String,
    auteur: Option<String>,
    publie_le: Option<String>,
    nb_commentaires: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BilletParams {
    blog_id: String,
    billet_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Commentaires {
    commentaires: Vec<CommentaireSortie>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct CommentaireSortie {
    id: String,
    auteur: Option<String>,
    texte: String,
    date: Option<String>,
    /// `PUBLISHED` ou `SUBMITTED` (en attente de modération).
    etat: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CommenterParams {
    blog_id: String,
    billet_id: String,
    /// Commentaire en texte brut.
    texte: String,
    /// Répondre à ce commentaire (facultatif).
    #[serde(default)]
    en_reponse_a: Option<String>,
}

#[allow(clippy::struct_field_names)] // noms de paramètres lus par l'assistant, explicites à dessein
#[derive(Debug, Deserialize, JsonSchema)]
struct SupprimerParams {
    blog_id: String,
    billet_id: String,
    commentaire_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Fait {
    fait: bool,
    reponse_ent: Value,
}

impl Ent1Server {
    fn billet(&self, b: &Value) -> BilletSortie {
        BilletSortie {
            id: identifiant(b),
            titre: champ(b, &["title"]).unwrap_or_default(),
            contenu: html_vers_texte(&champ(b, &["content"]).unwrap_or_default()),
            auteur: b.get("author").and_then(|a| champ(a, &["username"])),
            publie_le: ["firstPublishDate", "modified", "created"]
                .iter()
                .find_map(|k| b.get(*k))
                .and_then(|d| self.date_valeur(d)),
            nb_commentaires: b.get("nbComments").and_then(Value::as_u64),
        }
    }
}

#[tool_router(router = blog_router, vis = "pub(super)")]
impl Ent1Server {
    #[tool(
        description = "Blogs accessibles (blogs de classe, d'école…).",
        annotations(title = "Blogs", read_only_hint = true, open_world_hint = false)
    )]
    async fn blog_lister(&self) -> Sortie<Blogs> {
        let brut: Vec<Value> = self.ent.get("/blog/list/all").await?;
        Ok(Json(Blogs {
            blogs: brut
                .iter()
                .map(|b| BlogSortie {
                    id: identifiant(b),
                    titre: champ(b, &["title"]).unwrap_or_default(),
                    description: html_vers_texte(&champ(b, &["description"]).unwrap_or_default()),
                    auteur: b.get("author").and_then(|a| champ(a, &["username"])),
                    modifie_le: b.get("modified").and_then(|d| self.date_valeur(d)),
                    commentaires: champ(b, &["comment-type"]),
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Billets publiés d'un blog, du plus récent au plus ancien, avec leur contenu.",
        annotations(
            title = "Billets d'un blog",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn blog_billets(&self, Parameters(p): Parameters<BilletsParams>) -> Sortie<Billets> {
        let c = chemin(&["/blog/post/list/all/", &p.blog_id])?;
        let page = p.page.to_string();
        let query = [
            ("page", page.as_str()),
            ("content", "true"),
            ("comments", "false"),
            ("nbComments", "true"),
        ];
        let brut: Vec<Value> = self.ent.get_avec(&c, Some(&query)).await?;
        Ok(Json(Billets {
            billets: brut.iter().map(|b| self.billet(b)).collect(),
        }))
    }

    #[tool(
        description = "Commentaires d'un billet de blog.",
        annotations(
            title = "Commentaires d'un billet",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn blog_commentaires(
        &self,
        Parameters(p): Parameters<BilletParams>,
    ) -> Sortie<Commentaires> {
        let c = chemin(&["/blog/comments/", &p.blog_id, "/", &p.billet_id])?;
        let brut: Vec<Value> = self.ent.get(&c).await?;
        Ok(Json(Commentaires {
            commentaires: brut
                .iter()
                .map(|v| CommentaireSortie {
                    id: identifiant(v),
                    auteur: v.get("author").and_then(|a| champ(a, &["username"])),
                    texte: html_vers_texte(&champ(v, &["comment"]).unwrap_or_default()),
                    date: v.get("created").and_then(|d| self.date_valeur(d)),
                    etat: champ(v, &["state"]),
                })
                .collect(),
        }))
    }

    #[tool(
        description = "Publie un commentaire sous un billet de blog. Faire valider le texte par l'utilisateur.",
        annotations(
            title = "Commenter un billet",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn blog_commenter(&self, Parameters(p): Parameters<CommenterParams>) -> Sortie<Fait> {
        let c = chemin(&["/blog/comment/", &p.blog_id, "/", &p.billet_id])?;
        let corps = json!({ "comment": p.texte, "replyTo": p.en_reponse_a });
        let v: Value = self
            .ent
            .ecrire(Method::POST, &c, Corps::Json(&corps))
            .await?;
        Ok(Json(Fait {
            fait: true,
            reponse_ent: v,
        }))
    }

    #[tool(
        description = "Supprime un de vos commentaires sur un billet de blog.",
        annotations(
            title = "Supprimer un commentaire",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn blog_supprimer_commentaire(
        &self,
        Parameters(p): Parameters<SupprimerParams>,
    ) -> Sortie<Fait> {
        let c = chemin(&[
            "/blog/comment/",
            &p.blog_id,
            "/",
            &p.billet_id,
            "/",
            &p.commentaire_id,
        ])?;
        let v: Value = self
            .ent
            .ecrire::<Value, ()>(Method::DELETE, &c, Corps::Aucun)
            .await?;
        Ok(Json(Fait {
            fait: true,
            reponse_ent: v,
        }))
    }
}
