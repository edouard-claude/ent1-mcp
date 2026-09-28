//! Conversions HTML ⇄ texte.
//!
//! L'ENT stocke messages, mots et actualités en HTML. Un assistant lit
//! mieux du texte brut, et un parent écrit du texte brut.

/// Largeur de rendu : large, pour laisser le client gérer le retour à la ligne.
const LARGEUR: usize = 10_000;

/// HTML de l'ENT → texte lisible. Ne panique jamais : en cas d'échec du
/// rendu, on retombe sur un retrait grossier des balises.
pub fn html_vers_texte(html: &str) -> String {
    match html2text::from_read(html.as_bytes(), LARGEUR) {
        Ok(t) => nettoyer(&t),
        Err(_) => nettoyer(&retirer_balises(html)),
    }
}

/// Texte brut → HTML sûr : tout est échappé, les paragraphes (lignes vides)
/// deviennent des `<p>` et les retours simples des `<br>`.
pub fn texte_vers_html(texte: &str) -> String {
    let normalise = texte.replace("\r\n", "\n");
    let mut html = String::with_capacity(normalise.len() + 16);
    for paragraphe in normalise
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        html.push_str("<p>");
        for (i, ligne) in paragraphe.lines().enumerate() {
            if i > 0 {
                html.push_str("<br>");
            }
            html.push_str(&html_escape::encode_text(ligne));
        }
        html.push_str("</p>");
    }
    html
}

fn nettoyer(t: &str) -> String {
    let mut sortie = String::with_capacity(t.len());
    let mut vides = 0;
    for ligne in t.lines().map(str::trim_end) {
        if ligne.is_empty() {
            vides += 1;
            if vides > 1 {
                continue;
            }
        } else {
            vides = 0;
        }
        sortie.push_str(ligne);
        sortie.push('\n');
    }
    sortie.trim().to_owned()
}

fn retirer_balises(html: &str) -> String {
    let mut sortie = String::with_capacity(html.len());
    let mut dans_balise = false;
    for c in html.chars() {
        match c {
            '<' => dans_balise = true,
            '>' => dans_balise = false,
            _ if !dans_balise => sortie.push(c),
            _ => {}
        }
    }
    html_escape::decode_html_entities(&sortie).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echappe_et_structure() {
        let html = texte_vers_html("Bonjour <madame>,\nmerci.\n\nCordialement");
        assert_eq!(
            html,
            "<p>Bonjour &lt;madame&gt;,<br>merci.</p><p>Cordialement</p>"
        );
    }

    #[test]
    fn rend_du_texte() {
        let t = html_vers_texte("<p>Sortie <b>jeudi</b></p><p>&eacute;t&eacute;</p>");
        assert!(t.contains("Sortie"));
        assert!(t.contains("jeudi"));
        assert!(t.contains("été"));
        assert!(!t.contains('<'));
    }

    #[test]
    fn repli_sans_balises() {
        assert_eq!(nettoyer(&retirer_balises("<p>a</p>&amp;")), "a&");
    }
}
