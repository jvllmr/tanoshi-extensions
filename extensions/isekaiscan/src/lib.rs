use anyhow::bail;
use madara::{
    get_chapters, get_latest_manga, get_manga_detail, get_pages, get_popular_manga, search_manga,
};
use tanoshi_lib::prelude::{Extension, Lang, PluginRegistrar, SourceInfo};

tanoshi_lib::export_plugin!(register);

fn register(registrar: &mut dyn PluginRegistrar) {
    registrar.register_function(Box::new(IsekaiScan::default()));
}

const ID: i64 = 18;
const NAME: &str = "IsekaiScan.com";
const URL: &str = "https://isekaiscan.top";

#[derive(Default)]
pub struct IsekaiScan;

impl Extension for IsekaiScan {
    fn get_source_info(&self) -> SourceInfo {
        SourceInfo {
            id: ID,
            name: NAME.to_string(),
            url: URL.to_string(),
            version: env!("CARGO_PKG_VERSION"),
            icon: "https://i.imgur.com/7fsmq6F.png",
            languages: Lang::Single("en".to_string()),
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
        let IsekaiScan = IsekaiScan::default();

        let res1 = IsekaiScan.get_latest_manga(1).unwrap();
        assert!(!res1.is_empty());

        let res2 = IsekaiScan.get_latest_manga(2).unwrap();
        assert!(!res2.is_empty());

        assert_ne!(
            res1[0].path, res2[0].path,
            "{} should be different than {}",
            res1[0].path, res2[0].path
        );
    }

    #[test]
    fn test_get_popular_manga() {
        let IsekaiScan = IsekaiScan::default();

        let res = IsekaiScan.get_popular_manga(1).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_search_manga() {
        let IsekaiScan = IsekaiScan::default();

        let res = IsekaiScan
            .search_manga(1, Some("the+only".to_string()), None)
            .unwrap();

        assert!(!res.is_empty());
    }

    #[test]
    fn test_get_manga_detail() {
        let IsekaiScan = IsekaiScan::default();

        let res = IsekaiScan
            .get_manga_detail("/manga/infinite-level-up-in-murim/".to_string())
            .unwrap();

        assert_eq!(res.title, "Infinite Level Up in Murim");
    }

    #[test]
    fn test_get_chapters() {
        let IsekaiScan = IsekaiScan::default();

        let res = IsekaiScan
            .get_chapters("/manga/infinite-level-up-in-murim/".to_string())
            .unwrap();

        assert!(!res.is_empty());
        println!("{res:?}");
    }

    #[test]
    fn test_get_pages() {
        let IsekaiScan = IsekaiScan::default();

        let res = IsekaiScan
            .get_pages("/manga/infinite-level-up-in-murim/chapter-116/".to_string())
            .unwrap();

        assert!(!res.is_empty());
    }
}
