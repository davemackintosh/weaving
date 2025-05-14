pub mod globals;
use futures::StreamExt;
use globals::LiquidGlobals;
use std::sync::Arc;

use markdown::{ParseOptions, mdast::Node};
use slug::slugify;
use tokio::sync::Mutex;

use crate::document::Heading;
use crate::{BuildError, document::Document};

pub enum TemplateRenderer {
    LiquidBuilder {
        liquid_parser: liquid::Parser,
        weaver_template: Arc<Mutex<crate::Template>>,
    },
}

impl TemplateRenderer {
    pub fn new(template: Arc<Mutex<crate::Template>>) -> Self {
        Self::LiquidBuilder {
            liquid_parser: liquid::ParserBuilder::with_stdlib().build().unwrap(),
            weaver_template: template.clone(),
        }
    }

    pub async fn render(&self, data: &LiquidGlobals) -> Result<String, BuildError> {
        match self {
            Self::LiquidBuilder {
                liquid_parser,
                weaver_template,
            } => {
                let wtemplate = weaver_template.lock().await;

                match liquid_parser
                    .parse(&wtemplate.contents)
                    .unwrap()
                    .render(&data.to_liquid_data())
                {
                    Ok(result) => Ok(result),
                    Err(err) => Err(BuildError::Err(err.to_string())),
                }
            }
        }
    }
}

pub struct MarkdownRenderer {
    document: Arc<Mutex<Document>>,
    templates: Arc<Vec<Arc<Mutex<crate::Template>>>>,
}

impl MarkdownRenderer {
    pub fn new(
        document: Arc<Mutex<Document>>,
        templates: Arc<Vec<Arc<Mutex<crate::Template>>>>,
    ) -> Self {
        Self {
            document,
            templates,
        }
    }

    // Helper function to recursively extract text from inline nodes
    // This is needed to get the raw text content of a heading or other inline structures
    fn extract_text_from_mdast_inline(node: &Node) -> String {
        let mut text = String::new();
        match &node {
            Node::Text(text_node) => text.push_str(&text_node.value),
            Node::Code(code_node) => text.push_str(&code_node.value),
            // Add other inline node types you want to include text from (e.g., Strong, Emphasis, Link)
            // These nodes typically have children, so we need to recurse
            Node::Emphasis(_) | Node::Strong(_) | Node::Link(_) => {
                if let Some(children) = node.children() {
                    for child in children.iter() {
                        text.push_str(&Self::extract_text_from_mdast_inline(child)); // Recurse
                    }
                }
            }
            _ => {
                // For other node types, if they have children, recurse into them
                if let Some(children) = node.children() {
                    for child in children.iter() {
                        text.push_str(&Self::extract_text_from_mdast_inline(child));
                    }
                }
            }
        }
        text
    }

    fn collect_mdast_headings_to_map(node: &Node, headings_map: &mut Vec<Heading>) {
        // Check if the current node is a Heading
        if let Node::Heading(heading) = &node {
            let heading_text = if let Some(children) = node.children() {
                let mut text = String::new();
                for child in children.iter() {
                    text.push_str(&Self::extract_text_from_mdast_inline(child));
                }
                text
            } else {
                String::new()
            };
            let slug = slugify(&heading_text);
            if !slug.is_empty() {
                headings_map.push(Heading {
                    slug,
                    text: heading_text,
                    depth: heading.depth,
                });
            }
        }

        // Recursively visit children of the current node.
        // Headings can appear as children of Root, BlockQuote, List, ListItem, etc.
        if let Some(children) = node.children() {
            for child in children.iter() {
                Self::collect_mdast_headings_to_map(child, headings_map);
            }
        }
    }

    fn toc_from_document(&self, document: &Document) -> Vec<Heading> {
        let mut toc_map = vec![];
        let ast = markdown::to_mdast(document.markdown.as_str(), &ParseOptions::gfm()).unwrap();
        Self::collect_mdast_headings_to_map(&ast, &mut toc_map);
        toc_map
    }

    async fn find_template_by_string(
        &self,
        template_name: String,
    ) -> Option<&Arc<Mutex<crate::Template>>> {
        futures::stream::iter(self.templates.iter())
            .filter(|&t| {
                let name = template_name.clone();
                Box::pin(
                    async move { t.lock().await.at_path.ends_with(format!("{}.liquid", name)) },
                )
            })
            .next()
            .await
    }

    pub async fn render(&self, data: &mut LiquidGlobals) -> Result<String, BuildError> {
        let mut doc_guard = self.document.lock().await;
        let template = self
            .find_template_by_string(doc_guard.metadata.template.clone())
            .await
            .unwrap();

        doc_guard.toc = self.toc_from_document(&doc_guard);

        let markdown_html =
            markdown::to_html_with_options(doc_guard.markdown.as_str(), &markdown::Options::gfm())
                .expect("failed to render markdown to html");
        let template_renderer = TemplateRenderer::new(template.clone());
        data.page.body = Some(markdown_html);

        template_renderer.render(&data.to_owned()).await
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::template::Template;

    use super::*;

    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_liquid() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/liquid", base_path_wd);
        let template = Template::new_from_path(format!("{}/template.liquid", base_path).into());
        let renderer = TemplateRenderer::new(Arc::new(Mutex::new(template)));
        let doc_arc = Arc::new(Mutex::new(Document::new_from_path(
            format!("{}/test_fixtures/markdown/with_headings.md", base_path_wd).into(),
        )));

        let data = LiquidGlobals::new(doc_arc, &HashMap::new()).await;

        assert_eq!(
            "<!doctype html>
<html>
	<head>
		<title>test</title>
	</head>
	<body></body>
</html>
",
            renderer.render(&data).await.unwrap()
        );
    }

    #[test]
    fn test_markdown_toc_generation() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/markdown", base_path_wd);
        let doc_arc = Arc::new(Mutex::new(Document::new_from_path(
            format!("{}/with_headings.md", base_path).into(),
        )));
        let renderer = MarkdownRenderer::new(doc_arc.clone(), vec![].into());

        assert_eq!(
            vec![
                Heading {
                    depth: 1,
                    text: "heading 1".into(),
                    slug: "heading-1".into(),
                },
                Heading {
                    depth: 2,
                    text: "heading 2".into(),
                    slug: "heading-2".into(),
                },
                Heading {
                    depth: 3,
                    text: "heading 3".into(),
                    slug: "heading-3".into(),
                },
                Heading {
                    depth: 4,
                    text: "heading 4".into(),
                    slug: "heading-4".into(),
                },
                Heading {
                    depth: 5,
                    text: "heading 5".into(),
                    slug: "heading-5".into(),
                },
                Heading {
                    depth: 6,
                    text: "heading 6".into(),
                    slug: "heading-6".into(),
                },
            ],
            renderer.toc_from_document(&Document::new_from_path(
                format!("{}/with_headings.md", base_path).into(),
            ))
        );
    }
}
