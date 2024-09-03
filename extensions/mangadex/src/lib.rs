mod dto;
mod filter;

use crate::dto::{
    manga::{request, ListOrder, Order, Rating},
    Relationship, Results,
};
use anyhow::{anyhow, bail, Result};
use dto::ResultsAtHome;
use fancy_regex::Regex;
use tanoshi_lib::prelude::*;
use tanoshi_lib::extensions::PluginRegistrar;
use lazy_static::lazy_static;
use networking::{Agent, build_ureq_agent};
use std::env;

tanoshi_lib::export_plugin!(register);

fn register(registrar: &mut dyn PluginRegistrar) {
    registrar.register_function(Box::new(Mangadex::default()));
}

lazy_static! {
    static ref PREFERENCES: Vec<Input> = vec![];
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub static ID: i64 = 2;
pub static NAME: &str = "Mangadex";
pub static URL: &str = "https://api.mangadex.org";

pub struct Mangadex {
    preferences: Vec<Input>,
    client: Agent,
}

impl Default for Mangadex {
    fn default() -> Self {
        Self {
            preferences: PREFERENCES.clone(),
            client: build_ureq_agent(None, None),
        }
    }
}

#[must_use]
fn remove_bbcode(string: String) -> String {
    let regex = Regex::new(r#"\[(\w+)[^]]*](.*?)\[/\1]"#).unwrap();

    let result = string
        .replace("[list]", "")
        .replace("[/list]", "")
        .replace("[*]", "")
        .replace("[hr]", "\n");

    regex.replace_all(&result, "$2").to_string()
}

pub fn map_tags_to_string(relationships: Vec<Relationship>) -> Vec<String> {
    let mut tags = vec![];
    for relationship in relationships {
        if let Relationship::Tag { attributes, .. } = relationship {
            if let Some(name) = attributes.and_then(|attr| attr.name.get("en").cloned()) {
                tags.push(name.to_owned());
            }
        };
    }

    tags
}

pub fn map_result_to_manga(data: Relationship) -> Option<MangaInfo> {
    match data {
        Relationship::Manga {
            id,
            attributes,
            relationships,
        } => {
            let mut author = vec![];
            let mut genre = vec![];
            let mut file_name = "".to_string();
            for relationship in relationships {
                match relationship {
                    Relationship::Author { attributes, .. } => {
                        if let Some(name) = attributes.map(|attr| attr.name) {
                            author.push(name);
                        }
                    }
                    Relationship::Artist { attributes, .. } => {
                        if let Some(name) = attributes.map(|attr| attr.name) {
                            author.push(name);
                        }
                    }
                    Relationship::Tag { attributes, .. } => {
                        if let Some(name) = attributes.and_then(|attr| attr.name.get("en").cloned())
                        {
                            genre.push(name.to_owned());
                        }
                    }
                    Relationship::CoverArt { attributes, .. } => {
                        if let Some(name) = attributes.map(|attr| attr.file_name) {
                            file_name = name;
                        }
                    }
                    _ => {}
                };
            }

            Some(MangaInfo {
                source_id: ID,
                title: attributes
                    .clone()
                    .and_then(|attr| {
                        if let Some(title) = attr.title.get("en").cloned() {
                            Some(title)
                        } else if let Some(title) = attr.title.get("ja-ro").cloned() {
                            Some(title)
                        } else if let Some(title) = attr.title.get("ja").cloned() {
                            Some(title)
                        } else {
                            attr.title.values().next().cloned()
                        }
                    })
                    .unwrap_or_else(String::new),
                author,
                genre: attributes
                    .clone()
                    .map(|attr| attr.tags)
                    .map(map_tags_to_string)
                    .unwrap_or_else(Vec::new),
                status: attributes
                    .clone()
                    .and_then(|attr| attr.status)
                    .map(|s| s.to_string()),
                description: attributes
                    .and_then(|attr| attr.description.get("en").cloned())
                    .map(remove_bbcode),
                path: format!("/manga/{}", id),
                cover_url: format!("https://uploads.mangadex.org/covers/{}/{}", id, file_name),
            })
        }
        _ => None,
    }
}

pub fn map_result_to_chapter(data: Relationship) -> Option<ChapterInfo> {
    match data {
        Relationship::Chapter {
            id,
            attributes,
            relationships,
        } => {
            let mut scanlator = "".to_string();
            for relationship in relationships {
                if let Relationship::ScanlationGroup { attributes, .. } = relationship {
                    if let Some(name) = attributes.map(|attr| attr.name) {
                        scanlator = name;
                    }
                }
            }

            let volume = attributes.clone().and_then(|attr| attr.volume);
            let number = attributes.clone().and_then(|attr| attr.chapter);
            let mut title = attributes
                .clone()
                .and_then(|attr| attr.title)
                .unwrap_or_else(|| "".to_string());

            if title.is_empty() {
                if let Some(vol) = volume {
                    title = format!("Volume {}", vol);
                }
                if let Some(ch) = number.clone() {
                    title = format!("{} Chapter {}", title, ch)
                }
                title = title.trim().to_string();
            }

            Some(ChapterInfo {
                source_id: ID,
                title,
                path: format!("/chapter/{}", id),
                number: number
                    .and_then(|chapter| chapter.parse().ok())
                    .unwrap_or_default(),
                scanlator: Some(scanlator),
                uploaded: attributes
                    .map(|attr| attr.publish_at.naive_utc().timestamp())
                    .unwrap_or_else(|| 0),
            })
        }
        _ => None,
    }
}

pub fn map_result_to_pages(data: ResultsAtHome) -> Vec<String> {
    data.chapter
        .data
        .iter()
        .map(|d| format!("{}/data/{}/{}", data.base_url, data.chapter.hash, d))
        .collect()
}

impl Mangadex {
    fn get_manga_list(&self, mut page: i64, query: request::MangaList) -> Result<Vec<MangaInfo>> {
        if page < 1 {
            page = 1;
        }
        let offset = (page - 1) * 20;
        let query = request::MangaList {
            limit: 20,
            offset,
            ..query
        };

        let url = format!("{}/manga?{}", URL, query.to_query_string()?);

        let res: Results = self.client.get(&url).call()?.into_json()?;
        if let dto::Data::Multiple { data, .. } = res.data {
            Ok(data.into_iter().filter_map(map_result_to_manga).collect())
        } else {
            bail!("invalid data");
        }
    }
}

impl Extension for Mangadex {
    fn set_preferences(
        &mut self,
        preferences: Vec<Input>,
    ) -> anyhow::Result<()> {
        for input in preferences {
            for pref in self.preferences.iter_mut() {
                if input.eq(pref) {
                    *pref = input.clone();
                }
            }
        }

        Ok(())
    }

    fn get_preferences(&self) -> anyhow::Result<Vec<Input>> {
        Ok(self.preferences.clone())
    }

    fn get_source_info(&self) -> SourceInfo {
        SourceInfo {
            id: ID,
            name: NAME.to_string(),
            url: URL.to_string(),
            version: VERSION,
            icon: "https://mangadex.org/favicon.ico",
            languages: Lang::All,
            nsfw: true,
        }
    }

    fn get_popular_manga(&self, page: i64) -> anyhow::Result<Vec<MangaInfo>> {
        let query = request::MangaList {
            order: Some(ListOrder {
                followed_count: Some(Order::Desc),
                ..Default::default()
            }),
            ..Default::default()
        };
        self.get_manga_list(page, query)
    }

    fn get_latest_manga(&self, page: i64) -> anyhow::Result<Vec<MangaInfo>> {
        self.get_manga_list(page, request::MangaList::default())
    }

    fn search_manga(
        &self,
        page: i64,
        query: Option<String>,
        filters: Option<Vec<Input>>,
    ) -> anyhow::Result<Vec<MangaInfo>> {
        let query_list = if let Some(filters) = filters {
            filters.into()
        } else if let Some(query) = query {
            request::MangaList {
                title: Some(query),
                content_rating: vec![Rating::Safe, Rating::Suggestive, Rating::Erotica, Rating::Pornographic],
                ..Default::default()
            }
        } else {
            bail!("query and filters cannot be both empty")
        };

        self.get_manga_list(page, query_list)
    }

    fn get_manga_detail(&self, path: String) -> anyhow::Result<MangaInfo> {        
        let url = format!(
            "{}{}?includes[]=author&includes[]=artist&includes[]=cover_art",
            URL, path
        );

        let res: Results = self.client.get(&url).call()?.into_json()?;
        if let dto::Data::Single { data, .. } = res.data {
            map_result_to_manga(data).ok_or_else(|| anyhow!("no such manga"))
        } else {
            bail!("invalid data");
        }
    }

    fn get_chapters(&self, path: String) -> anyhow::Result<Vec<ChapterInfo>> {        
        let url = format!(
            "{}{}/feed?limit=500&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic&translatedLanguage[]=en&includes[]=scanlation_group",
            URL, path
        );

        let res: Results = self.client.get(&url).call()?.into_json()?;
        if let dto::Data::Multiple { data, .. } = res.data {
            Ok(data.into_iter().filter_map(map_result_to_chapter).collect())
        } else {
            bail!("invalid data");
        }
    }

    fn get_pages(&self, path: String) -> anyhow::Result<Vec<String>> {        
        let chapter_id = path.replace("/chapter/", "");
        let url = format!("{}/at-home/server/{}", URL, chapter_id);

        let res: ResultsAtHome = self.client.get(&url).call()?.into_json()?;
        Ok(map_result_to_pages(res))
    }

    fn headers(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    fn filter_list(&self) -> Vec<Input> {
        filter::FILTER_LIST.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_latest_manga() {
        let mangadex = Mangadex::default();

        let res = mangadex.get_latest_manga(1).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_popular_manga() {
        let mangadex = Mangadex::default();

        let res = mangadex.get_popular_manga(1).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_search_manga() {
        let mangadex = Mangadex::default();

        let res = mangadex
            .search_manga(1, Some("komi".to_string()), None)
            .unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_manga_detail() {
        let mangadex = Mangadex::default();

        let res = mangadex
            .get_manga_detail("/manga/a96676e5-8ae2-425e-b549-7f15dd34a6d8".to_string())
            .unwrap();
        assert_eq!(res.title, "Komi-san wa Komyushou Desu.");
    }

    #[test]
    fn test_get_chapters() {
        let mangadex = Mangadex::default();

        let res = mangadex
            .get_chapters("/manga/a96676e5-8ae2-425e-b549-7f15dd34a6d8".to_string())
            .unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_pages() {
        let mangadex = Mangadex::default();

        let res = mangadex
            .get_pages("/chapter/03d3e4b9-db8d-4fb5-88fc-b6a087bd6410".to_string())
            .unwrap();

        assert!(!res.is_empty());
    }
}
