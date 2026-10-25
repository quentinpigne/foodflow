//! Lecture de la table Ciqual au format XML (R-ALI-50).
//!
//! Les fichiers de l'ANSES sont encodés en Windows-1252 et ne sont pas du XML valide : certaines teneurs
//! contiennent un `<` brut (« < 0,5 »). Leur structure étant très régulière (un enregistrement par balise,
//! un champ par balise), on les lit champ par champ plutôt qu'avec un analyseur XML strict.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// Les trois fichiers utiles d'une table Ciqual, trouvés dans un dossier (`alim_*`, `alim_grp_*`, `compo_*`).
pub struct CiqualFiles {
    pub foods: PathBuf,
    pub groups: PathBuf,
    pub compositions: PathBuf,
}

impl CiqualFiles {
    pub fn find(dir: &Path) -> anyhow::Result<Self> {
        let mut foods = None;
        let mut groups = None;
        let mut compositions = None;
        for entry in std::fs::read_dir(dir).with_context(|| format!("lecture de {}", dir.display()))? {
            let path = entry?.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_lowercase();
            if !name.ends_with(".xml") {
                continue;
            }
            if name.starts_with("alim_grp_") {
                groups = Some(path);
            } else if name.starts_with("alim_") {
                foods = Some(path);
            } else if name.starts_with("compo_") {
                compositions = Some(path);
            }
        }
        match (foods, groups, compositions) {
            (Some(foods), Some(groups), Some(compositions)) => Ok(Self { foods, groups, compositions }),
            _ => bail!("dossier incomplet : il faut les fichiers alim_*.xml, alim_grp_*.xml et compo_*.xml de Ciqual"),
        }
    }
}

/// Lit un fichier Ciqual (Windows-1252) en texte.
pub fn read(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("lecture de {}", path.display()))?;
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
    Ok(text.into_owned())
}

/// Corps des enregistrements `<TAG> … </TAG>`, dans l'ordre.
fn records<'a>(text: &'a str, tag: &str) -> impl Iterator<Item = &'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut rest = text;
    std::iter::from_fn(move || {
        let start = rest.find(&open)? + open.len();
        let end = start + rest[start..].find(&close)?;
        let body = &rest[start..end];
        rest = &rest[end + close.len()..];
        Some(body)
    })
}

/// Valeur d'un champ `<name> valeur </name>` d'un enregistrement, espaces retirés.
fn field<'a>(record: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = record.find(&open)? + open.len();
    let end = start + record[start..].find(&close)?;
    Some(record[start..end].trim())
}

fn required<'a>(record: &'a str, name: &str) -> anyhow::Result<&'a str> {
    field(record, name).with_context(|| format!("champ {name} manquant"))
}

/// Code de la famille des aliments sans groupe dans la table.
pub const UNCLASSIFIED: &str = "00";

/// Un niveau de l'arborescence des groupes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiqualGroup {
    pub code: String,
    pub name: String,
    pub parent_code: Option<String>,
}

/// Ciqual écrit les noms de groupes en minuscules : on met une majuscule initiale.
fn capitalize(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Code absent : « 000000 », « 0000 » ou « - ».
fn present(code: &str) -> bool {
    !code.is_empty() && code.chars().any(|c| c != '0' && c != '-')
}

/// Groupes, sous-groupes et sous-sous-groupes, parents avant enfants, sans doublon (R-ALI-51).
pub fn parse_groups(text: &str) -> anyhow::Result<Vec<CiqualGroup>> {
    let mut groups: Vec<CiqualGroup> = Vec::new();
    let mut push = |code: &str, name: &str, parent: Option<&str>| {
        if present(code) && !groups.iter().any(|g| g.code == code) {
            groups.push(CiqualGroup {
                code: code.to_owned(),
                name: capitalize(name),
                parent_code: parent.map(str::to_owned),
            });
        }
    };
    for record in records(text, "ALIM_GRP") {
        let grp = required(record, "alim_grp_code")?;
        let ssgrp = required(record, "alim_ssgrp_code")?;
        let ssssgrp = required(record, "alim_ssssgrp_code")?;
        push(grp, required(record, "alim_grp_nom_fr")?, None);
        push(ssgrp, required(record, "alim_ssgrp_nom_fr")?, Some(grp));
        push(ssssgrp, required(record, "alim_ssssgrp_nom_fr")?, Some(if present(ssgrp) { ssgrp } else { grp }));
    }
    Ok(groups)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiqualFood {
    pub code: String,
    pub name: String,
    /// Groupes de l'aliment, du plus fin au plus large. La table référence parfois des sous-groupes absents
    /// de son fichier de groupes : l'import prend le plus fin qui existe.
    pub group_codes: Vec<String>,
}

pub fn parse_foods(text: &str) -> anyhow::Result<Vec<CiqualFood>> {
    records(text, "ALIM")
        .map(|record| {
            let codes = [
                required(record, "alim_ssssgrp_code")?,
                required(record, "alim_ssgrp_code")?,
                required(record, "alim_grp_code")?,
            ];
            let mut group_codes: Vec<String> = codes.into_iter().filter(|c| present(c)).map(str::to_owned).collect();
            // Quelques aliments n'ont aucun groupe (« Dessert (aliment moyen) ») : famille « Non classés ».
            if group_codes.is_empty() {
                group_codes.push(UNCLASSIFIED.to_owned());
            }
            Ok(CiqualFood {
                code: required(record, "alim_code")?.to_owned(),
                name: required(record, "alim_nom_fr")?.to_owned(),
                group_codes,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiqualValue {
    pub food_code: String,
    pub constituent_code: String,
    /// Teneur telle qu'écrite dans la table (« 0,12 », « traces », « < 0,5 », « - »).
    pub raw: String,
}

/// Teneurs des constituants demandés.
pub fn parse_compositions(text: &str, constituents: &[&str]) -> anyhow::Result<Vec<CiqualValue>> {
    let mut values = Vec::new();
    for record in records(text, "COMPO") {
        let constituent = required(record, "const_code")?;
        if constituents.contains(&constituent) {
            values.push(CiqualValue {
                food_code: required(record, "alim_code")?.to_owned(),
                constituent_code: constituent.to_owned(),
                raw: required(record, "teneur")?.to_owned(),
            });
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ciqual").join(name);
        read(&path).unwrap()
    }

    #[test]
    fn r_ali_51_groups_tree() {
        let groups = parse_groups(&fixture("alim_grp_extrait.xml")).unwrap();
        let laits = groups.iter().find(|g| g.code == "0501").unwrap();
        assert_eq!(laits.name, "Laits");
        assert_eq!(laits.parent_code.as_deref(), Some("05"));
        assert_eq!(groups.iter().find(|g| g.code == "05").unwrap().name, "Produits laitiers et assimilés");
        assert!(!groups.iter().any(|g| g.code == "000000"), "les codes absents sont ignorés");
    }

    #[test]
    fn foods_take_their_finest_group() {
        let foods = parse_foods(&fixture("alim_extrait.xml")).unwrap();
        let milk = foods.iter().find(|f| f.code == "19041").unwrap();
        assert_eq!(milk.name, "Lait demi-écrémé, UHT");
        assert_eq!(milk.group_codes, ["050101", "0501", "05"]);
    }

    /// Les teneurs « < x » ne sont pas du XML valide : elles se lisent quand même (R-ALI-21).
    #[test]
    fn r_ali_21_compositions_with_raw_less_than() {
        let values = parse_compositions(&fixture("compo_extrait.xml"), &["328", "34100", "10004"]).unwrap();
        assert!(values.iter().any(|v| v.raw.starts_with('<')));
        assert!(values.iter().any(|v| v.raw == "-"));
        assert!(values.iter().all(|v| ["328", "34100", "10004"].contains(&v.constituent_code.as_str())));
    }
}
