use std::env;

use anyhow::bail;
use lazy_static::lazy_static;
use madara::{
    get_chapters, get_manga_detail, get_pages,
    parse_manga_list,
};
use networking::{build_flaresolverr_client, build_ureq_agent, Agent};
use scraper::{Selector};
use tanoshi_lib::prelude::{Extension, Input, Lang, PluginRegistrar, SourceInfo};
tanoshi_lib::export_plugin!(register);

fn register(registrar: &mut dyn PluginRegistrar) {
    registrar.register_function(Box::new(ThreeSixtyFiveManga::default()));
}

lazy_static! {
    static ref PREFERENCES: Vec<Input> = vec![];
}

const ID: i64 = 17;
const NAME: &str = "365Manga";
const URL: &str = "https://harimanga.com";

pub struct ThreeSixtyFiveManga {
    preferences: Vec<Input>,
    client: Agent,
}

impl Default for ThreeSixtyFiveManga {
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

impl Extension for ThreeSixtyFiveManga {
    fn set_preferences(&mut self, preferences: Vec<Input>) -> anyhow::Result<()> {
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
            version: env!("CARGO_PKG_VERSION"),
            icon: "https://i.imgur.com/q1r31vg.png",
            languages: Lang::Single("en".to_string()),
            nsfw: false,
        }
    }

    fn get_popular_manga(&self, page: i64) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        let body = self
            .client
            .post(&format!("{}/manga/page/{}/?m_orderby=trending", URL, page))
            .set("Referer", URL)
            .set("X-Requested-With", "XMLHttpRequest")
            .call()
            .unwrap()
            .into_string()
            .unwrap();

        let selector = Selector::parse("div.page-item-detail")
            .map_err(|e| anyhow::anyhow!("failed to parse selector: {:?}", e))?;

        parse_manga_list(URL, ID, &body, &selector, false)
    }

    fn get_latest_manga(&self, page: i64) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        let body = self
            .client
            .post(&format!("{}/manga/page/{}/?m_orderby=latest", URL, page))
            .set("Referer", URL)
            .set("X-Requested-With", "XMLHttpRequest")
            .call()
            .unwrap()
            .into_string()
            .unwrap();

        let selector = Selector::parse("div.page-item-detail")
            .map_err(|e| anyhow::anyhow!("failed to parse selector: {:?}", e))?;

        parse_manga_list(URL, ID, &body, &selector, false)
    }

    fn search_manga(
        &self,
        page: i64,
        query: Option<String>,
        _: Option<Vec<Input>>,
    ) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        if let Some(query) = query {
            let body = self
                .client
                .post(&format!(
                    "{}/page/{}/?s={}&post_type=wp-manga",
                    URL, page, query
                ))
                .set("Referer", URL)
                .set("X-Requested-With", "XMLHttpRequest")
                .call()
                .unwrap()
                .into_string()
                .unwrap();

            let selector = Selector::parse("div.c-tabs-item > div")
                .map_err(|e| anyhow::anyhow!("failed to parse selector: {:?}", e))?;

            parse_manga_list(URL, ID, &body, &selector, false)
        } else {
            bail!("query can not be empty")
        }
    }

    fn get_manga_detail(&self, path: String) -> anyhow::Result<tanoshi_lib::prelude::MangaInfo> {
        get_manga_detail(URL, &path, ID, &self.client)
    }

    fn get_chapters(&self, path: String) -> anyhow::Result<Vec<tanoshi_lib::prelude::ChapterInfo>> {
        get_chapters(URL, &path, ID, None, &self.client)
    }

    fn get_pages(&self, path: String) -> anyhow::Result<Vec<String>> {
        get_pages(URL, &path, &self.client)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn create_test_instance() -> ThreeSixtyFiveManga {
        let preferences: Vec<Input> = vec![];

        let mut three_sixty_five_manga: ThreeSixtyFiveManga = ThreeSixtyFiveManga::default();

        three_sixty_five_manga.set_preferences(preferences).unwrap();

        three_sixty_five_manga
    }

    #[test]
    fn test_get_latest_manga() {
        let three_sixty_five_manga: ThreeSixtyFiveManga = create_test_instance();

        let res1 = three_sixty_five_manga.get_latest_manga(1).unwrap();
        assert!(!res1.is_empty());

        let res2 = three_sixty_five_manga.get_latest_manga(2).unwrap();
        assert!(!res2.is_empty());

        assert_ne!(
            res1[0].path, res2[0].path,
            "{} should be different than {}",
            res1[0].path, res2[0].path
        );
    }

    #[test]
    fn test_get_popular_manga() {
        let three_sixty_five_manga: ThreeSixtyFiveManga = create_test_instance();

        let res = three_sixty_five_manga.get_popular_manga(1).unwrap();
        assert!(!res.is_empty());
    }

    #[ignore = "Probably Cloudflare issues"]
    #[test]
    fn test_search_manga() {
        let three_sixty_five_manga: ThreeSixtyFiveManga = create_test_instance();

        let res = three_sixty_five_manga
            .search_manga(1, Some("the+only".to_string()), None)
            .unwrap();

        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_manga_detail() {
        let three_sixty_five_manga: ThreeSixtyFiveManga = create_test_instance();

        let res = three_sixty_five_manga
            .get_manga_detail("/manga/how-to-make-a-loving-savior-an-emperor/".to_string())
            .unwrap();

        assert_eq!(res.title, "How To Make A Loving Savior an Emperor");
    }

    #[test]
    fn test_get_chapters() {
        let three_sixty_five_manga: ThreeSixtyFiveManga = create_test_instance();

        let res = three_sixty_five_manga
            .get_chapters("/manga/how-to-make-a-loving-savior-an-emperor/".to_string())
            .unwrap();

        assert!(!res.is_empty());
        println!("{res:?}");
    }

    #[test]
    fn test_get_pages() {
        let three_sixty_five_manga: ThreeSixtyFiveManga = create_test_instance();

        let res = three_sixty_five_manga
            .get_pages("/manga/how-to-make-a-loving-savior-an-emperor/chapter-9/".to_string())
            .unwrap();

        assert!(!res.is_empty());
    }
}
