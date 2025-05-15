pub mod globals;
use async_trait::async_trait;
use futures::StreamExt;
use globals::LiquidGlobals;
use std::path::PathBuf;
use std::sync::Arc;

use markdown::{ParseOptions, mdast::Node};
use slug::slugify;
use tokio::sync::Mutex;

use crate::document::Heading;
use crate::filters::raw_html::RawHtml;
use crate::routes::route_from_path;
use crate::{BuildError, document::Document};

#[derive(Debug, PartialEq)]
pub struct WritableFile {
    pub contents: String,
    pub path: PathBuf,
}

#[async_trait]
pub trait ContentRenderer {
    async fn render(&self, data: &mut LiquidGlobals) -> Result<WritableFile, BuildError>;
}

fn out_path_for_document(document: &Document, weaver_config: &Arc<crate::WeaverConfig>) -> PathBuf {
    let out_base = weaver_config.build_dir.clone();
    let document_content_path = route_from_path(
        weaver_config.content_dir.clone().into(),
        document.at_path.clone().into(),
    );

    format!("{}{}index.html", out_base, document_content_path).into()
}

pub enum TemplateRenderer<'a> {
    LiquidBuilder {
        liquid_parser: liquid::Parser,
        for_document: &'a Document,
        weaver_template: Arc<Mutex<crate::Template>>,
        weaver_config: Arc<crate::WeaverConfig>,
    },
}

#[async_trait]
impl<'a> ContentRenderer for TemplateRenderer<'a> {
    async fn render(&self, data: &mut LiquidGlobals) -> Result<WritableFile, BuildError> {
        match self {
            Self::LiquidBuilder {
                liquid_parser,
                weaver_template,
                for_document,
                weaver_config,
            } => {
                let wtemplate = weaver_template.lock().await;

                match liquid_parser
                    .parse(&wtemplate.contents)
                    .unwrap()
                    .render(&data.to_liquid_data())
                {
                    Ok(result) => Ok(WritableFile {
                        contents: result,
                        path: out_path_for_document(&for_document, weaver_config),
                    }),
                    Err(err) => Err(BuildError::Err(err.to_string())),
                }
            }
        }
    }
}

impl<'a> TemplateRenderer<'a> {
    pub fn new(
        template: Arc<Mutex<crate::Template>>,
        for_document: &'a Document,
        weaver_config: Arc<crate::WeaverConfig>,
    ) -> Self {
        Self::LiquidBuilder {
            liquid_parser: liquid::ParserBuilder::with_stdlib()
                .filter(RawHtml)
                .build()
                .unwrap(),
            weaver_template: template.clone(),
            for_document,
            weaver_config,
        }
    }
}

pub struct MarkdownRenderer {
    document: Arc<Mutex<Document>>,
    templates: Arc<Vec<Arc<Mutex<crate::Template>>>>,
    weaver_config: Arc<crate::WeaverConfig>,
}

#[async_trait]
impl ContentRenderer for MarkdownRenderer {
    async fn render(&self, data: &mut LiquidGlobals) -> Result<WritableFile, BuildError> {
        let mut doc_guard = self.document.lock().await;
        let template = self
            .find_template_by_string(doc_guard.metadata.template.clone())
            .await
            .unwrap();

        doc_guard.toc = self.toc_from_document(&doc_guard);

        let markdown_html =
            markdown::to_html_with_options(doc_guard.markdown.as_str(), &markdown::Options::gfm())
                .expect("failed to render markdown to html");
        let template_renderer =
            TemplateRenderer::new(template.clone(), &doc_guard, self.weaver_config.clone());
        data.page.body = markdown_html;

        template_renderer.render(&mut data.to_owned()).await
    }
}

impl MarkdownRenderer {
    pub fn new(
        document: Arc<Mutex<Document>>,
        templates: Arc<Vec<Arc<Mutex<crate::Template>>>>,
        weaver_config: Arc<crate::WeaverConfig>,
    ) -> Self {
        Self {
            document,
            templates,
            weaver_config,
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
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::{config::WeaverConfig, template::Template};

    use super::*;

    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_liquid() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/example", base_path_wd);
        let template = Template::new_from_path(
            format!("{}/test_fixtures/liquid/template.liquid", base_path_wd).into(),
        );
        let doc_arc =
            Document::new_from_path(format!("{}/content/with_headings.md", base_path).into());
        let config = Arc::new(WeaverConfig::new_from_path(base_path.clone().into()));
        let renderer =
            TemplateRenderer::new(Arc::new(Mutex::new(template)), &doc_arc, config.clone());

        let mut data = LiquidGlobals::new(
            Arc::new(Mutex::new(Document::new_from_path(
                format!("{}/content/with_headings.md", base_path).into(),
            ))),
            &HashMap::new(),
        )
        .await;

        assert_eq!(
            WritableFile {
                contents: "<!doctype html>
<html>
	<head>
		<title>test</title>
	</head>
	<body></body>
</html>
"
                .into(),
                path: format!("{}/site/with_headings/index.html", base_path).into(),
            },
            renderer.render(&mut data).await.unwrap()
        );
    }

    #[test]
    fn test_markdown_toc_generation() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/markdown", base_path_wd);
        let doc_arc = Arc::new(Mutex::new(Document::new_from_path(
            format!("{}/with_headings.md", base_path).into(),
        )));
        let config_path = format!("{}/test_fixtures/config/custom_config", base_path_wd);
        let config = Arc::new(WeaverConfig::new_from_path(config_path.clone().into()));
        let renderer = MarkdownRenderer::new(doc_arc.clone(), vec![].into(), config.clone());

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

    #[tokio::test]
    async fn test_render() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/example", base_path_wd);
        let template =
            Template::new_from_path(format!("{}/templates/default.liquid", base_path).into());
        let doc_arc = Arc::new(Mutex::new(Document::new_from_path(
            format!("{}/content/with_headings.md", base_path).into(),
        )));
        let config = Arc::new(WeaverConfig::new_from_path(base_path.clone().into()));
        let renderer = MarkdownRenderer::new(
            doc_arc.clone(),
            vec![Arc::new(Mutex::new(template))].into(),
            config.clone(),
        );

        let mut data = LiquidGlobals::new(doc_arc, &HashMap::new()).await;
        let result = renderer.render(&mut data).await;

        assert_eq!(
            WritableFile {
                contents: r#"<!doctype html>
<html lang="en">
	<head>
		<meta charset="utf-8" />

		<title>test</title>
		<link rel="icon" href="/static/favicon.ico" />
		<meta name="viewport" content="width=device-width, initial-scale=1" />

		<meta name="description" content="test"/>
		<meta name="keywords" content="test"/>
	</head>
	<body>
		<main>
			<h1>test</h1>
			<article>
				<h1>heading 1</h1>
<p>I am a paragraph.</p>
<h2>heading &lt;span&gt;2&lt;/span&gt;</h2>
<p>I'm the second paragraph.</p>
<h3>heading 3</h3>
<h4>heading 4</h4>
<h5>heading 5</h5>
<h6>heading 6</h6>
			</article>
		</main>
	</body>
</html>

"#
                .into(),
                path: format!("{}/site/with_headings/index.html", base_path).into()
            },
            result.unwrap()
        );
    }
}
