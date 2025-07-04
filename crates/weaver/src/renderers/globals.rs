use crate::config::WeaverConfig;
use crate::document::{BaseMetaData, Heading};
use crate::routes::route_from_path;
use liquid::model::KString;
use liquid::{self};
use serde::{Deserialize, Serialize};
use std::path::{Component, PathBuf};
use std::{collections::HashMap, sync::Arc};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
pub struct LiquidGlobalsPage {
    pub route: KString,
    pub title: String,
    pub body: String,
    pub meta: BaseMetaData,
    pub toc: Vec<Heading>,
}

impl LiquidGlobalsPage {
    pub fn to_liquid_data(&self) -> liquid::model::Value {
        liquid::model::to_value(self)
            .expect("Failed to serialize LiquidGlobalsPage to liquid value")
    }
}

impl From<&crate::Document> for LiquidGlobalsPage {
    fn from(value: &crate::Document) -> Self {
        Self {
            route: route_from_path(value.content_root.clone(), value.at_path.clone().into()).into(),
            meta: value.metadata.clone(),
            body: value.html.clone().unwrap_or("".into()),
            toc: value.toc.clone(),
            title: value.metadata.title.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LiquidGlobals {
    pub page: LiquidGlobalsPage,
    pub content: HashMap<KString, Vec<LiquidGlobalsPage>>,
    pub extra_css: String,
    pub site_config: Arc<WeaverConfig>,
}

type ContentMap = HashMap<KString, Vec<LiquidGlobalsPage>>;

impl LiquidGlobals {
    pub async fn new(
        page_arc_mutex: Arc<tokio::sync::Mutex<crate::Document>>,
        all_documents_by_route: &Arc<HashMap<KString, LiquidGlobalsPage>>,
        site_config: Arc<WeaverConfig>,
    ) -> Self {
        let page_guard = page_arc_mutex.lock().await;
        let page_globals = LiquidGlobalsPage::from(&*page_guard);

        let mut content_map: ContentMap = HashMap::new();
        for (route, doc_arc_mutex) in all_documents_by_route.iter() {
            let path = PathBuf::from(route);
            let mut components = path.components().peekable();

            if route == &page_globals.route {
                continue;
            }

            let first_component = if let Some(Component::RootDir) = components.peek() {
                components.next() // Skip the leading '/'
            } else {
                None
            }
            .and_then(|_| components.next()) // Get the next component after root (if any)
            .map(|c| {
                if let Component::Normal(os_str) = c {
                    KString::from(os_str.to_string_lossy().into_owned())
                } else {
                    KString::from("root")
                }
            });

            if first_component.is_none() {
                content_map.insert(route.clone(), vec![doc_arc_mutex.clone()]);
            } else {
                let f_path = first_component.unwrap();
                match content_map.contains_key(&f_path) {
                    true => {
                        // Don't include the "list" page in the content list.
                        if route.clone() != format!("/{}/", f_path) {
                            let content_inner_map = content_map.get_mut(&f_path).unwrap();
                            content_inner_map.push(doc_arc_mutex.clone());
                        }
                    }
                    false => {
                        content_map.insert(f_path.clone(), vec![doc_arc_mutex.clone()]);
                    }
                }
            }
        }

        drop(page_guard);

        let content = content_map
            .into_iter()
            .map(|(key, mut content)| {
                content.sort_by(|a, b| b.meta.published.cmp(&a.meta.published));
                (key, content)
            })
            .collect::<HashMap<KString, Vec<LiquidGlobalsPage>>>();

        Self {
            page: page_globals,
            content,
            extra_css: "".into(),
            site_config,
        }
    }

    pub fn to_liquid_data(&self) -> liquid::Object {
        liquid::object!({
            "page": self.page.to_liquid_data(),
            "extra_css": self.extra_css,
            "content": liquid::model::to_value(&self.content)
                 .expect("Failed to serialize content HashMap to liquid value"),
            "site_config": liquid::model::to_value(&*self.site_config)
                 .expect("Failed to serialize site config to liquid value"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquid::ValueView;
    use liquid::model::KString;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn create_mock_document(route: &str, title: &str, body: Option<&str>) -> crate::Document {
        crate::Document {
            content_root: PathBuf::new(),
            at_path: route.to_string(),
            metadata: BaseMetaData {
                title: title.to_string(),
                ..Default::default()
            },
            emit: true,
            html: body.map(|s| s.to_string()),
            markdown: String::new(),
            toc: vec![],
        }
    }

    #[test]
    fn test_liquid_globals_page_to_liquid_data() {
        let liquid_page = LiquidGlobalsPage {
            route: KString::from("/test"),
            title: "Test Page".to_string(),
            body: "<p>Test Body</p>".to_string(),
            toc: vec![],
            meta: BaseMetaData {
                title: "Test Meta Title".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let liquid_value = liquid_page.to_liquid_data();

        assert!(liquid_value.is_object());
        let liquid_object = liquid_value.as_object().unwrap();

        assert_eq!(
            liquid_object
                .get(&KString::from("route"))
                .unwrap()
                .as_scalar()
                .unwrap()
                .to_kstr(),
            "/test"
        );
        assert_eq!(
            liquid_object
                .get(&KString::from("title"))
                .unwrap()
                .as_scalar()
                .unwrap()
                .to_kstr(),
            "Test Page"
        );
        assert_eq!(
            liquid_object
                .get(&KString::from("body"))
                .unwrap()
                .as_scalar()
                .unwrap()
                .to_kstr(),
            "<p>Test Body</p>"
        );

        let meta_value = liquid_object.get(&KString::from("meta")).unwrap();
        assert!(meta_value.is_object());
        let meta_object = meta_value.as_object().unwrap();
        assert_eq!(
            meta_object
                .get(&KString::from("title"))
                .unwrap()
                .as_scalar()
                .unwrap()
                .to_kstr(),
            "Test Meta Title"
        );
        assert_eq!(
            meta_object
                .get(&KString::from("template"))
                .unwrap()
                .as_scalar()
                .unwrap()
                .to_kstr(),
            "default"
        );

        /*let expected_tags_liquid_array = liquid::model::Value::Array(vec![
            liquid::model::Value::scalar("tag1"),
            liquid::model::Value::scalar("tag2"),
        ]);
        assert_eq!(
            meta_object
                .get(&KString::from("tags"))
                .unwrap()
                .as_array()
                .unwrap(),
            &expected_tags_liquid_array
        );

        let expected_keywords_liquid_array = liquid::model::Value::Array(vec![]);
        assert_eq!(
            meta_object.get(&KString::from("keywords")).unwrap(), // Get &LiquidValue
            &expected_keywords_liquid_array // Compare with expected &LiquidValue::Array
        );*/
    }

    #[tokio::test]
    async fn test_liquid_globals_new() {
        let page_doc = create_mock_document("/page", "Page Title", Some("<p>page body</p>"));
        let content_doc_1 =
            create_mock_document("/posts/post-1", "Post One", Some("<p>post 1 body</p>"));
        let content_doc_2 = create_mock_document("/about", "About Us", None);

        let page_arc_mutex = Arc::new(Mutex::new(page_doc.clone()));
        let post1_arc_mutex = Arc::new(Mutex::new(content_doc_1.clone()));
        let about_arc_mutex = Arc::new(Mutex::new(content_doc_2.clone()));

        let mut all_documents_by_route = HashMap::new();
        all_documents_by_route.insert(KString::from("/page"), LiquidGlobalsPage::from(&page_doc));
        all_documents_by_route.insert(
            KString::from("/posts/post-1"),
            LiquidGlobalsPage::from(&content_doc_1),
        );
        all_documents_by_route.insert(
            KString::from("/about"),
            LiquidGlobalsPage::from(&content_doc_2),
        );

        let liquid_globals = LiquidGlobals::new(
            Arc::clone(&page_arc_mutex),
            &Arc::new(all_documents_by_route),
            Arc::new(WeaverConfig::default()),
        )
        .await;

        let page_doc_guard = page_arc_mutex.lock().await;
        let expected_page_globals = LiquidGlobalsPage::from(&*page_doc_guard);
        assert_eq!(liquid_globals.page, expected_page_globals);
        drop(page_doc_guard);

        assert_eq!(liquid_globals.content.len(), 3);

        assert!(liquid_globals.content.contains_key(&KString::from("posts")));
        assert!(
            liquid_globals
                .content
                .get("posts")
                .unwrap()
                .iter()
                .any(|p| p.route == "/posts/post-1/")
        );
        assert!(liquid_globals.content.contains_key(&KString::from("about")));

        let post1_doc_guard = post1_arc_mutex.lock().await;
        let expected_post1_globals = LiquidGlobalsPage::from(&*post1_doc_guard);
        assert_eq!(
            liquid_globals
                .content
                .get(&KString::from("posts"))
                .unwrap()
                .iter()
                .find(|p| p.route == "/posts/post-1/")
                .unwrap(),
            &expected_post1_globals
        );
        drop(post1_doc_guard);

        let about_doc_guard = about_arc_mutex.lock().await;
        let expected_about_globals = LiquidGlobalsPage::from(&*about_doc_guard);
        assert_eq!(
            liquid_globals
                .content
                .get(&KString::from("about"))
                .unwrap()
                .iter()
                .find(|p| p.route == "/about/")
                .unwrap(),
            &expected_about_globals
        );
        drop(about_doc_guard);
    }

    #[tokio::test]
    async fn test_liquid_globals_new_only_page_doc() {
        let page_doc = create_mock_document("/index", "Home Page", Some("<p>home</p>"));
        let page_arc_mutex = Arc::new(Mutex::new(page_doc.clone()));
        let page_global = LiquidGlobalsPage::from(&page_doc);

        let mut all_documents_by_route = HashMap::new();
        all_documents_by_route.insert(KString::from("/index"), page_global);

        let liquid_globals = LiquidGlobals::new(
            Arc::clone(&page_arc_mutex),
            &Arc::new(all_documents_by_route),
            Arc::new(WeaverConfig::default()),
        )
        .await;

        let page_doc_guard = page_arc_mutex.lock().await;
        let expected_page_globals = LiquidGlobalsPage::from(&*page_doc_guard);
        assert_eq!(liquid_globals.page, expected_page_globals);
        drop(page_doc_guard);

        assert_eq!(liquid_globals.content.len(), 1);
    }

    #[test]
    fn test_liquid_globals_to_liquid_data() {
        let page_page = LiquidGlobalsPage {
            route: KString::from("/page"),
            title: "Page".to_string(),
            body: "<p>page</p>".to_string(),
            meta: BaseMetaData {
                title: "Page Meta".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let content_page_1 = LiquidGlobalsPage {
            route: KString::from("/post-1"),
            title: "Post 1".to_string(),
            body: "<p>post1</p>".to_string(),
            meta: BaseMetaData {
                title: "Post 1 Meta".to_string(),
                ..Default::default()
            },
            toc: vec![],
            ..Default::default()
        };
        let content_page_2 = LiquidGlobalsPage {
            route: KString::from("/about"),
            title: "About".to_string(),
            body: "".into(),
            meta: BaseMetaData {
                title: "About Meta".to_string(),
                ..Default::default()
            },
            toc: vec![],
            ..Default::default()
        };

        let mut content_map: HashMap<KString, Vec<LiquidGlobalsPage>> = HashMap::new();
        content_map.insert(KString::from("/post-1"), vec![content_page_1.clone()]);
        content_map.insert(KString::from("/about"), vec![content_page_2.clone()]);

        let liquid_globals = LiquidGlobals {
            page: page_page.clone(),
            content: content_map.clone(),
            extra_css: "".into(),
            site_config: Arc::new(WeaverConfig::default()),
        };

        let liquid_object = liquid_globals.to_liquid_data();

        assert!(liquid_object.is_object());
        let liquid_map = liquid_object.as_object().unwrap();

        assert!(liquid_map.contains_key(&KString::from("page")));
        assert!(liquid_map.contains_key(&KString::from("content")));
        assert!(liquid_map.contains_key(&KString::from("extra_css")));
        assert!(liquid_map.contains_key(&KString::from("site_config")));
        assert_eq!(liquid_map.size(), 4);

        /*let page_value = liquid_map.get(&KString::from("page")).unwrap();
        let expected_page_liquid_value = page_page.to_liquid_data();
        assert_eq!(page_value, &expected_page_liquid_value);

        let content_value = liquid_map.get(&KString::from("content")).unwrap();
        assert!(content_value.is_object());
        let expected_content_liquid_value = liquid::model::to_value(&content_map)
            .expect("Failed to serialize expected content map");
        assert_eq!(content_value, &expected_content_liquid_value);

        let content_object = content_value.as_object().unwrap();
        let post1_liquid_value = content_object.get(&KString::from("/post-1")).unwrap();
        assert!(post1_liquid_value.is_object());
        let post1_object = post1_liquid_value.as_object().unwrap();
        assert_eq!(
            post1_object
                .get(&KString::from("title"))
                .unwrap()
                .as_scalar()
                .unwrap()
                .to_kstr(),
            "Post 1"
        );

        let about_liquid_value = content_object.get(&KString::from("/about")).unwrap();
        assert!(about_liquid_value.is_object());
        let about_object = about_liquid_value.as_object().unwrap();
        assert_eq!(
            about_object
                .get(&KString::from("route"))
                .unwrap()
                .as_scalar()
                .unwrap()
                .to_kstr(),
            "/about"
        );*/
    }
}
