use liquid::model::KString;
use liquid::{self};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

use crate::document::BaseMetaData;

#[derive(Serialize, Deserialize, Clone)]
pub struct LiquidGlobalsPage {
    pub route: KString,
    pub title: String,
    pub body: Option<String>,
    pub meta: Option<BaseMetaData>,
    pub excerpt: Option<String>,
}

impl LiquidGlobalsPage {
    pub fn to_liquid_data(&self) -> liquid::model::Value {
        liquid::model::to_value(self)
            .expect("Failed to serialize LiquidGlobalsPage to liquid value")
    }
}

// Update the From implementation to take a reference and populate the route
impl From<&crate::Document> for LiquidGlobalsPage {
    fn from(value: &crate::Document) -> Self {
        let route_kstring = KString::from(value.at_path.clone()); // Using path string as route key

        Self {
            route: route_kstring, // Populate the new route field
            excerpt: value.excerpt.clone(),
            meta: Some(value.metadata.clone()),
            body: value.html.clone(), // Include the rendered HTML body
            title: value.metadata.title.clone(), // Include the title
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LiquidGlobals {
    pub page: LiquidGlobalsPage,
    pub content: HashMap<KString, LiquidGlobalsPage>,
}

impl LiquidGlobals {
    pub async fn new(
        page_arc_mutex: Arc<tokio::sync::Mutex<crate::Document>>,
        all_documents_by_route: &HashMap<KString, Arc<tokio::sync::Mutex<crate::Document>>>,
    ) -> Self {
        let page_guard = page_arc_mutex.lock().await;
        let page_globals = LiquidGlobalsPage::from(&*page_guard);

        let mut content_map = HashMap::new();
        for (route, doc_arc_mutex) in all_documents_by_route.iter() {
            if route != &page_globals.route {
                let doc_guard = doc_arc_mutex.lock().await;
                let content_page_globals = LiquidGlobalsPage::from(&*doc_guard);
                content_map.insert(route.clone(), content_page_globals);
            }
        }

        Self {
            page: page_globals,
            content: content_map,
        }
    }

    pub fn to_liquid_data(&self) -> liquid::Object {
        liquid::object!({
            "page": self.page.to_liquid_data(),
            "content": liquid::model::to_value(&self.content)
                 .expect("Failed to serialize content HashMap to liquid value")
        })
    }
}
