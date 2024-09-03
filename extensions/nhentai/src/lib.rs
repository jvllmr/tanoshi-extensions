use anyhow::{anyhow, Result};
use chrono::NaiveDateTime;
use fancy_regex::Regex;
use scraper::{Html, Selector};
use tanoshi_lib::prelude::{
    ChapterInfo, Extension, Input, InputType, Lang, MangaInfo, PluginRegistrar,
};
use lazy_static::lazy_static;
use networking::{Agent, build_ureq_agent, build_flaresolverr_client};
use std::env;

pub static ID: i64 = 6;
pub static NAME: &str = "nhentai";
pub static URL: &str = "https://nhentai.net";

tanoshi_lib::export_plugin!(register);

fn register(registrar: &mut dyn PluginRegistrar) {
    registrar.register_function(Box::new(NHentai::default()));
}

lazy_static! {
    static ref TAG_FILTER: Input = Input::Text {
        name: "Tag".to_string(),
        state: None
    };
    static ref CHARACTERS_FILTER: Input = Input::Text {
        name: "Characters".to_string(),
        state: None
    };
    static ref ARTISTS_FILTER: Input = Input::Text {
        name: "Artists".to_string(),
        state: None
    };
    static ref GROUPS_FILTER: Input = Input::Text {
        name: "Groups".to_string(),
        state: None
    };
    static ref CATEGORIES_FILTER: Input = Input::Text {
        name: "Categories".to_string(),
        state: None
    };
    static ref PARODIES_FILTER: Input = Input::Text {
        name: "Parodies".to_string(),
        state: None
    };
    static ref SORT_FILTER: Input = Input::Select {
        name: "Sort".to_string(),
        values: vec![
            InputType::String("Popular".to_string()),
            InputType::String("Popular Week".to_string()),
            InputType::String("Popular Today".to_string()),
            InputType::String("Recent".to_string()),
        ],
        state: None
    };
    static ref FILTER_LIST: Vec<Input> = vec![
        TAG_FILTER.clone(),
        CHARACTERS_FILTER.clone(),
        CATEGORIES_FILTER.clone(),
        PARODIES_FILTER.clone(),
        ARTISTS_FILTER.clone(),
        GROUPS_FILTER.clone(),
        SORT_FILTER.clone()
    ];
    static ref LANGUAGE_SELECT: Input = Input::Select {
        name: "Language".to_string(),
        values: vec![
            InputType::String("Any".to_string()),
            InputType::String("English".to_string()),
            InputType::String("Japanese".to_string()),
            InputType::String("Chinese".to_string()),
        ],
        state: None
    };
    static ref BLACKLIST_TAG: Input = Input::Text {
        name: "Blacklist Tag".to_string(),
        state: None
    };
    
    static ref PREFERENCES: Vec<Input> = vec![LANGUAGE_SELECT.clone(), BLACKLIST_TAG.clone()];
}

pub struct NHentai {
    preferences: Vec<Input>,
    client: Agent,
}

impl Default for NHentai {
    fn default() -> Self {
        let mut instance = Self {
            preferences: PREFERENCES.clone(),
            client: build_ureq_agent(None, None),
        };

        // If flaresolverr_url is set, build the client with it
        if let Ok(flaresolverr_url) = env::var("FLARESOLVERR_URL") {
            instance.client = build_flaresolverr_client(URL, &flaresolverr_url).unwrap();
        }

        instance
    }
}

impl NHentai {
    fn query(&self, filters: Option<Vec<Input>>) -> String {
        let mut query = vec![];
        let mut sort = None;
        for pref in self.preferences.iter() {
            if LANGUAGE_SELECT.eq(pref) {
                if let Input::Select { state, values, .. } = pref {
                    if let Some(InputType::String(lang)) =
                        state.and_then(|index| values.get(index as usize))
                    {
                        if lang != "Any" {
                            query.push(format!("language:{}", lang.to_lowercase()));
                        }
                    }
                }
            } else if BLACKLIST_TAG.eq(pref) {
                if let Input::Text {
                    state: Some(state), ..
                } = pref
                {
                    for tag in state.split(',') {
                        query.push(format!("-tag:{}", tag.trim()))
                    }
                }
            }
        }

        if let Some(filters) = filters {
            for filter in filters.iter() {
                match filter {
                    Input::Text {
                        name,
                        state: Some(state),
                        ..
                    } if name == &TAG_FILTER.name() => {
                        for tag in state.split(',') {
                            if tag.starts_with('-') {
                                query.push(format!(
                                    "-{}:{}",
                                    name.to_lowercase(),
                                    tag.trim().replace("-", "")
                                ))
                            } else {
                                query.push(format!("{}:{}", name.to_lowercase(), tag.trim()))
                            }
                        }
                    }
                    Input::Text {
                        name,
                        state: Some(state),
                        ..
                    } => query.push(format!("{}:{}", name.to_lowercase(), state.trim())),
                    Input::Select {
                        name,
                        values,
                        state,
                        ..
                    } if name == &SORT_FILTER.name() => {
                        let state = state.unwrap_or(0);
                        if let Some(InputType::String(state)) = values.get(state as usize) {
                            sort = Some(format!("sort={}", state.replace(" ", "-").to_lowercase()));
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut query_str = if query.is_empty() {
            r#""""#.to_string()
        } else {
            query.join(" ")
        };

        if let Some(sort) = sort {
            query_str = format!("{query_str}&{sort}");
        }

        query_str
    }

    fn get_manga_list(&self, url: &str) -> Result<Vec<MangaInfo>> {       
        // Send the request and get the response as a string
        let res = self.client.get(&url)
            .call()?
            .into_string()?;
        
        let document = Html::parse_document(&res);
        let gallery_selector =
            Selector::parse(".gallery").map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let thumbnail_selector =
            Selector::parse("a > img").map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let path_selector =
            Selector::parse("a").map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let title_selector = Selector::parse("a > .caption")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;

        let mut manga_list = vec![];
        for gallery in document.select(&gallery_selector) {
            let cover_url = gallery
                .select(&thumbnail_selector)
                .flat_map(|thumbnail| thumbnail.value().attr("data-src"))
                .next()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("cover_url not found"))?;

            let path = gallery
                .select(&path_selector)
                .flat_map(|link| link.value().attr("href"))
                .next()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("path not found"))?;

            let title = gallery
                .select(&title_selector)
                .flat_map(|caption| caption.text().next())
                .next()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("title not found"))?;

            let manga = MangaInfo {
                source_id: ID,
                status: None,
                title,
                author: vec![],
                genre: vec![],
                description: None,
                path,
                cover_url,
            };

            manga_list.push(manga);
        }
        Ok(manga_list)
    }
}

impl Extension for NHentai {
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

    fn get_source_info(&self) -> tanoshi_lib::prelude::SourceInfo {
        tanoshi_lib::prelude::SourceInfo {
            id: ID,
            name: "NHentai".to_string(),
            url: URL.to_string(),
            version: env!("CARGO_PKG_VERSION"),
            icon: "https://static.nhentai.net/img/logo.090da3be7b51.svg",
            languages: Lang::Multi(vec!["en".to_string(), "jp".to_string(), "ch".to_string()]),
            nsfw: true,
        }
    }

    fn get_popular_manga(&self, page: i64) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        self.get_manga_list(&format!(
            "{URL}/search/?q={}&sort=popular&page={page}",
            self.query(None)
        ))
    }

    fn get_latest_manga(&self, page: i64) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        self.get_manga_list(&format!("{URL}/search/?q={}&page={page}", self.query(None)))
    }

    fn search_manga(
        &self,
        page: i64,
        query: Option<String>,
        filters: Option<Vec<Input>>,
    ) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        let url = if filters.is_some() {
            format!("{URL}/search/?q={}&page={page}", self.query(filters))
        } else if let Some(query) = query {
            format!("{URL}/search/?q={query}&sort=popular&page={page}")
        } else {
            return Err(anyhow!("query and filters cannot be both empty"));
        };

        self.get_manga_list(&url)
    }

    fn get_manga_detail(&self, path: String) -> anyhow::Result<tanoshi_lib::prelude::MangaInfo> {
        let url = format!("{}{}/", URL, path);
        // Send the request and get the response as a string
        let res = self.client.get(&url)
            .call()?
            .into_string()?;

        let document = Html::parse_document(&res);
        let gallery_id_selector = Selector::parse("h3[id=\"gallery_id\"]")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let parodies_selector = Selector::parse("a[href^=\"/parody/\"] > .name")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let characters_selector = Selector::parse("a[href^=\"/character/\"] > .name")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let languages_selector = Selector::parse("a[href^=\"/language/\"] > .name")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let categories_selector = Selector::parse("a[href^=\"/category/\"] > .name")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let pages_selector = Selector::parse("a[href^=\"/search/?q=pages\"] > .name")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let thumbnail_selector = Selector::parse("#cover > a > img")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let title_selector = Selector::parse("h1.title > .pretty")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let author_selector = Selector::parse("a[href^=\"/artist/\"] > .name")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let genre_selector = Selector::parse("a[href^=\"/tag/\"] > .name")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;

        let mut description = "".to_string();
        if let Some(gallery_id) = document.select(&gallery_id_selector).next().map(|el| {
            el.text()
                .into_iter()
                .map(|id| id.to_string())
                .collect::<Vec<String>>()
                .join("")
        }) {
            description = format!("{}", gallery_id);
        }
        let parodies = document
            .select(&parodies_selector)
            .into_iter()
            .filter_map(|el| el.text().next())
            .collect::<Vec<&str>>()
            .join(",");
        if !parodies.is_empty() {
            description = format!("{}\nParodies: {}", description, parodies);
        }
        let characters = document
            .select(&characters_selector)
            .into_iter()
            .filter_map(|el| el.text().next())
            .collect::<Vec<&str>>()
            .join(",");
        if !characters.is_empty() {
            description = format!("{}\nCharacters: {}", description, characters);
        }
        let languages = document
            .select(&languages_selector)
            .into_iter()
            .filter_map(|el| el.text().next())
            .collect::<Vec<&str>>()
            .join(",");
        if !languages.is_empty() {
            description = format!("{}\nLanguages: {}", description, languages);
        }
        let categories = document
            .select(&categories_selector)
            .into_iter()
            .filter_map(|el| el.text().next())
            .collect::<Vec<&str>>()
            .join(",");
        if !categories.is_empty() {
            description = format!("{}\nCategories: {}", description, categories);
        }
        if let Some(pages) = document.select(&pages_selector).next().map(|el| {
            el.text()
                .into_iter()
                .map(|id| id.to_string())
                .collect::<Vec<String>>()
                .join("")
        }) {
            description = format!("{}\nPages: {}", description, pages);
        }

        let cover_url = document
            .select(&thumbnail_selector)
            .flat_map(|el| el.value().attr("data-src"))
            .next()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("cover not found"))?;

        let title = document
            .select(&title_selector)
            .flat_map(|el| el.text())
            .next()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("title not found"))?;

        let author: Vec<String> = document
            .select(&author_selector)
            .flat_map(|el| el.text())
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        let genre: Vec<String> = document
            .select(&genre_selector)
            .flat_map(|el| el.text())
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        let manga = MangaInfo {
            source_id: ID,
            status: None,
            path,
            description: Some(description),
            title,
            author,
            genre,
            cover_url,
        };

        Ok(manga)
    }

    fn get_chapters(&self, path: String) -> anyhow::Result<Vec<tanoshi_lib::prelude::ChapterInfo>> {
        let url = format!("{}{}/", URL, path);

        // Send the request and get the response as a string
        let res = self.client.get(&url)
            .call()?
            .into_string()?;

        let document = Html::parse_document(&res);
        let scanlator_selector = Selector::parse("a[href^=\"/group/\"] > .name")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let uploaded_selector = Selector::parse(".tags > time")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;
        let scanlator = document
            .select(&scanlator_selector)
            .flat_map(|el| el.text())
            .next()
            .map(|s| s.to_string());
        let uploaded = if let Some(uploaded) = document.select(&uploaded_selector).next() {
            uploaded
                .value()
                .attr("datetime")
                .and_then(|t| NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.f%z").ok())
                .map(|dt| dt.timestamp())
        } else {
            None
        };

        let chapter = ChapterInfo {
            source_id: ID,
            title: "Chapter 1".to_string(),
            path,
            number: 1_f64,
            scanlator,
            uploaded: uploaded.unwrap_or_else(|| 0),
        };

        Ok(vec![chapter])
    }

    fn get_pages(&self, path: String) -> anyhow::Result<Vec<String>> {
        let url = format!("{}{}/", URL, path);

        // Send the request and get the response as a string
        let res = self.client.get(&url)
            .call()?
            .into_string()?;

        let document = Html::parse_document(&res);
        let page_selector = Selector::parse(".thumb-container > .gallerythumb > img")
            .map_err(|e| anyhow!("failed to parse selector: {e:?}"))?;

        let mut pages = vec![];
        let re = Regex::new(r"^https:\/\/t(\d*)\..+\/(\d+)\/(\d+)t.(.+)$")?;
        for thumb in document.select(&page_selector) {
            if let Some(url) = thumb.value().attr("data-src") {
                let cap = re
                    .captures(url)?
                    .ok_or(anyhow::anyhow!("no captured regex for {url}"))?;
                pages.push(format!(
                    "https://i{}.nhentai.net/galleries/{}/{}.{}",
                    &cap[1], &cap[2], &cap[3], &cap[4]
                ));
            }
        }

        Ok(pages)
    }

    fn headers(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    fn filter_list(&self) -> Vec<Input> {
        FILTER_LIST.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn create_test_instance() -> NHentai {
        let preferences: Vec<Input> = vec![
            Input::Text {
                name: "Blacklist Tag".to_string(),
                state: Some("posession".to_string()),
            },
            Input::Select {
                name: "Language".to_string(),
                values: vec![
                    InputType::String("Any".to_string()),
                    InputType::String("English".to_string()),
                    InputType::String("Japanese".to_string()),
                    InputType::String("Chinese".to_string()),
                ],
                state: Some(1),
            },
        ];

        let mut nhentai: NHentai = NHentai::default();
        
        nhentai.set_preferences(preferences).unwrap();

        nhentai
    }

    #[test]
    fn test_get_popular_manga() {
        let nhentai: NHentai = create_test_instance();

        let res = nhentai.get_popular_manga(1).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_latest_manga() {
        std::thread::sleep(std::time::Duration::from_secs(1));

        let nhentai: NHentai = create_test_instance();

        let res = nhentai.get_latest_manga(1).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_search_manga() {
        std::thread::sleep(std::time::Duration::from_secs(2));

        let nhentai: NHentai = create_test_instance();

        let res = nhentai
            .search_manga(1, Some("azur lane".to_string()), None)
            .unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_search_manga_filter() {
        std::thread::sleep(std::time::Duration::from_secs(3));

        let nhentai: NHentai = create_test_instance();

        let mut filters = nhentai.filter_list();
        for filter in filters.iter_mut() {
            if SORT_FILTER.eq(filter) {
                if let Input::Select { state, .. } = filter {
                    *state = Some(1);
                }
            } else if TAG_FILTER.eq(filter) {
                if let Input::Text { state, .. } = filter {
                    *state = Some("-big breasts".to_string());
                }
            } else if PARODIES_FILTER.eq(filter) {
                if let Input::Text { state, .. } = filter {
                    *state = Some("azur lane".to_string());
                }
            }
        }
        let res = nhentai.search_manga(1, None, Some(filters)).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_manga_detail() {
        let nhentai: NHentai = create_test_instance();

        let res = nhentai.get_manga_detail("/g/385965".to_string()).unwrap();

        assert_eq!(res.title, "Lady, Maid ni datsu");
    }

    #[test]
    fn test_get_chapters() {
        std::thread::sleep(std::time::Duration::from_secs(1));

        let nhentai: NHentai = create_test_instance();

        let res = nhentai.get_chapters("/g/385965".to_string()).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_pages() {
        std::thread::sleep(std::time::Duration::from_secs(2));

        let nhentai: NHentai = create_test_instance();

        let res = nhentai.get_pages("/g/385965".to_string()).unwrap();
        assert!(!res.is_empty());
        let re = Regex::new(r"https://i\d*.nhentai.net/galleries/2099700/1.jpg").unwrap();

        assert!(re.is_match(&res[0]).unwrap());
    }
}
