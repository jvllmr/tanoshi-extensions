use anyhow::bail;
use madara::{
    get_chapters, get_latest_manga, get_manga_detail, get_pages, get_popular_manga, search_manga,
};
use tanoshi_lib::prelude::{Extension, Lang, PluginRegistrar, SourceInfo};

tanoshi_lib::export_plugin!(register);

fn register(registrar: &mut dyn PluginRegistrar) {
    registrar.register_function(Box::new(LeviatanScan::default()));
}

const ID: i64 = 14;
const NAME: &str = "LeviatanScans";
const URL: &str = "https://en.leviatanscans.com";

#[derive(Default)]
pub struct LeviatanScan;

impl Extension for LeviatanScan {
    fn get_source_info(&self) -> SourceInfo {
        SourceInfo {
            id: ID,
            name: NAME.to_string(),
            url: URL.to_string(),
            version: env!("CARGO_PKG_VERSION"),
            icon: "https://i.imgur.com/UhrXXFv.png",
            languages: Lang::Multi(vec!["en".to_string(), "es".to_string()]),
            nsfw: false,
        }
    }

    fn get_popular_manga(&self, page: i64) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        get_popular_manga(URL, ID, page)
    }

    fn get_latest_manga(&self, page: i64) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        get_latest_manga(URL, ID, page)
    }

    fn search_manga(
        &self,
        page: i64,
        query: Option<String>,
        _: Option<Vec<tanoshi_lib::prelude::Input>>,
    ) -> anyhow::Result<Vec<tanoshi_lib::prelude::MangaInfo>> {
        if let Some(query) = query {
            search_manga(URL, ID, page, &query, false)
        } else {
            bail!("query can not be empty")
        }
    }

    fn get_manga_detail(&self, path: String) -> anyhow::Result<tanoshi_lib::prelude::MangaInfo> {
        get_manga_detail(URL, &path, ID)
    }

    fn get_chapters(&self, path: String) -> anyhow::Result<Vec<tanoshi_lib::prelude::ChapterInfo>> {
        get_chapters(URL, &path, ID, None)
    }

    fn get_pages(&self, path: String) -> anyhow::Result<Vec<String>> {
        get_pages(URL, &path)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_latest_manga() {
        let LeviatanScan = LeviatanScan::default();

        let res1 = LeviatanScan.get_latest_manga(1).unwrap();
        assert!(!res1.is_empty());

        let res2 = LeviatanScan.get_latest_manga(2).unwrap();
        assert!(!res2.is_empty());

        assert_ne!(
            res1[0].path, res2[0].path,
            "{} should be different than {}",
            res1[0].path, res2[0].path
        );
    }

    #[test]
    fn test_get_popular_manga() {
        let LeviatanScan = LeviatanScan::default();

        let res = LeviatanScan.get_popular_manga(1).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_search_manga() {
        let LeviatanScan = LeviatanScan::default();

        let res = LeviatanScan
            .search_manga(1, Some("the+only".to_string()), None)
            .unwrap();

        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_manga_detail() {
        let LeviatanScan = LeviatanScan::default();

        let res = LeviatanScan
            .get_manga_detail("/manga/survival-story-of-a-sword-king-in-a-fantasy-world-1/".to_string())
            .unwrap();

        assert_eq!(res.title, "Survival Story of a Sword King in a Fantasy World");
    }

    #[test]
    fn test_get_chapters() {
        let LeviatanScan = LeviatanScan::default();

        let res = LeviatanScan
            .get_chapters("/manga/survival-story-of-a-sword-king-in-a-fantasy-world-1/".to_string())
            .unwrap();
        assert!(!res.is_empty());
        println!("{res:?}");
    }

    #[test]
    fn test_get_pages() {
        let LeviatanScan = LeviatanScan::default();

        let res = LeviatanScan
            .get_pages("/manga/survival-story-of-a-sword-king-in-a-fantasy-world-1/chapter-1/".to_string())
            .unwrap();

        assert!(!res.is_empty());
    }
}
