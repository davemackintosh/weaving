pub mod globals;
use async_trait::async_trait;
use comrak::plugins::syntect::SyntectAdapterBuilder;
use comrak::{ExtensionOptions, Options, Plugins, RenderOptions, markdown_to_html_with_plugins};
use futures::StreamExt;
use globals::LiquidGlobals;
use liquid::partials::{EagerCompiler, InMemorySource};
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::config::TemplateLang;
use crate::filters::date_format::Date;
use crate::filters::has_key::HasKey;
use crate::filters::json::JSON;
use crate::filters::raw_html::RawHtml;
use crate::partial::Partial;
use crate::routes::route_from_path;
use crate::template::Template;
use crate::{BuildError, document::Document};

#[derive(Debug, PartialEq)]
pub struct WritableFile {
    pub contents: String,
    pub path: PathBuf,
    pub emit: bool,
}

#[async_trait]
pub trait ContentRenderer {
    async fn render(
        &self,
        data: &mut LiquidGlobals,
        partials: Vec<Partial>,
    ) -> Result<Option<WritableFile>, BuildError>;
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
    async fn render(
        &self,
        data: &mut LiquidGlobals,
        _partials: Vec<Partial>,
    ) -> Result<Option<WritableFile>, BuildError> {
        match self {
            Self::LiquidBuilder {
                liquid_parser,
                weaver_template,
                for_document,
                weaver_config,
            } => {
                let wtemplate = weaver_template.lock().await;

                match liquid_parser.parse(&wtemplate.contents) {
                    Ok(parsed) => match parsed.render(&data.to_liquid_data()) {
                        Ok(result) => Ok(Some(WritableFile {
                            contents: result,
                            path: out_path_for_document(for_document, weaver_config),
                            emit: for_document.emit,
                        })),
                        Err(err) => {
                            eprintln!(
                                "Template rendering error '{}' {:#?}",
                                &for_document.at_path, &err
                            );
                            Err(BuildError::Err(err.to_string()))
                        }
                    },
                    Err(err) => {
                        eprintln!(
                            "Template rendering error '{}' {:#?}",
                            &for_document.at_path, &err
                        );
                        Err(BuildError::Err(err.to_string()))
                    }
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
        partials: Vec<Partial>,
    ) -> Self {
        let mut registered_partials = EagerCompiler::<InMemorySource>::empty();

        for partial in partials {
            registered_partials.add(partial.name, partial.contents);
        }

        Self::LiquidBuilder {
            liquid_parser: liquid::ParserBuilder::with_stdlib()
                .filter(RawHtml)
                .filter(JSON)
                .filter(HasKey)
                .filter(Date)
                .partials(registered_partials)
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
    partials: Vec<Partial>,
}

// This renderer is strange for several reasons, the way it works is as follows.
// 1. Do a pass to gather the headings in the document
// 2. Do a pass over the template.
// 3. Do a pass over the markdown to get HTML from the
#[async_trait]
impl ContentRenderer for MarkdownRenderer {
    async fn render(
        &self,
        data: &mut LiquidGlobals,
        partials: Vec<Partial>,
    ) -> Result<Option<WritableFile>, BuildError> {
        let doc_guard = self.document.lock().await;
        let template = self
            .find_template_by_string(doc_guard.metadata.template.clone())
            .await
            .unwrap();

        let templated_md_html =
            Template::new_from_string(doc_guard.markdown.clone(), TemplateLang::Liquid);

        let body_template_renderer = TemplateRenderer::new(
            Arc::new(Mutex::new(templated_md_html)),
            &doc_guard,
            self.weaver_config.clone(),
            self.partials.clone(),
        );
        let body_html = body_template_renderer
            .render(&mut data.to_owned(), partials.clone())
            .await?;

        if body_html.is_none() {
            return Ok(None);
        }

        let mut markdown_plugins = Plugins::default();
        let markdown_syntax_hl_adapter = SyntectAdapterBuilder::new().css().build();
        markdown_plugins.render.codefence_syntax_highlighter = Some(&markdown_syntax_hl_adapter);
        let markdown_html = markdown_to_html_with_plugins(
            body_html.unwrap().contents.as_str(),
            &Options {
                render: RenderOptions {
                    unsafe_: true,
                    figure_with_caption: true,
                    gfm_quirks: true,
                    ..Default::default()
                },
                extension: ExtensionOptions {
                    strikethrough: true,
                    tagfilter: true,
                    table: true,
                    autolink: true,
                    header_ids: Some("".into()),
                    alerts: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            &markdown_plugins,
        );

        let template_renderer = TemplateRenderer::new(
            template.clone(),
            &doc_guard,
            self.weaver_config.clone(),
            partials.clone(),
        );
        data.page.body = markdown_html;

        template_renderer
            .render(&mut data.to_owned(), partials)
            .await
    }
}

impl MarkdownRenderer {
    pub fn new(
        document: Arc<Mutex<Document>>,
        templates: Arc<Vec<Arc<Mutex<crate::Template>>>>,
        weaver_config: Arc<crate::WeaverConfig>,
        partials: Vec<Partial>,
    ) -> Self {
        Self {
            document,
            templates,
            weaver_config,
            partials,
        }
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

    use crate::{config::WeaverConfig, normalize_line_endings, template::Template};

    use super::*;

    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_liquid() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/example", base_path_wd);
        let template = Template::new_from_path(
            format!("{}/test_fixtures/liquid/template.liquid", base_path_wd).into(),
        );
        let doc_arc = Document::new_from_path(
            base_path.clone().into(),
            format!("{}/content/with_headings.md", base_path).into(),
        );
        let config = Arc::new(WeaverConfig::new(base_path.clone().into()));
        let renderer = TemplateRenderer::new(
            Arc::new(Mutex::new(template)),
            &doc_arc,
            config.clone(),
            vec![],
        );

        let mut data = LiquidGlobals::new(
            Arc::new(Mutex::new(Document::new_from_path(
                base_path.clone().into(),
                format!("{}/content/with_headings.md", base_path).into(),
            ))),
            &Arc::new(HashMap::new()),
            Arc::new(WeaverConfig::default()),
        )
        .await;

        assert_eq!(
            WritableFile {
                contents: normalize_line_endings(
                    b"<!doctype html>
<html>
	<head>
		<title>test</title>
	</head>
	<body></body>
</html>
"
                ),
                path: format!("{}/site/with_headings/index.html", base_path).into(),
                emit: true,
            },
            renderer.render(&mut data, vec![]).await.unwrap().unwrap()
        );
    }

    #[tokio::test]
    async fn test_render() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/example", base_path_wd);
        let template =
            Template::new_from_path(format!("{}/templates/default.liquid", base_path).into());
        let doc_arc = Arc::new(Mutex::new(Document::new_from_path(
            base_path.clone().into(),
            format!("{}/content/with_headings.md", base_path).into(),
        )));
        let config = Arc::new(WeaverConfig::new(base_path.clone().into()));
        let renderer = MarkdownRenderer::new(
            doc_arc.clone(),
            vec![Arc::new(Mutex::new(template))].into(),
            config.clone(),
            vec![],
        );

        let mut data = LiquidGlobals::new(
            doc_arc,
            &Arc::new(HashMap::new()),
            Arc::new(WeaverConfig::default()),
        )
        .await;
        let result = renderer.render(&mut data, vec![]).await;

        assert_eq!(
            WritableFile {
                contents: normalize_line_endings(
                    br##"<!doctype html>
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
				<h1><a href="#heading-1" aria-hidden="true" class="anchor" id="heading-1"></a>heading 1</h1>
<p>I am a paragraph.</p>
<h2><a href="#heading-2" aria-hidden="true" class="anchor" id="heading-2"></a>heading <span>2</span></h2>
<p>I'm the second paragraph.</p>
<h3><a href="#heading-3" aria-hidden="true" class="anchor" id="heading-3"></a>heading 3</h3>
<h4><a href="#heading-4" aria-hidden="true" class="anchor" id="heading-4"></a>heading 4</h4>
<h5><a href="#heading-5" aria-hidden="true" class="anchor" id="heading-5"></a>heading 5</h5>
<h6><a href="#heading-6" aria-hidden="true" class="anchor" id="heading-6"></a>heading 6</h6>

			</article>
		</main>
	</body>
</html>
"##
                ),
                path: format!("{}/site/with_headings/index.html", base_path).into(),
                emit: true,
            },
            result.unwrap().unwrap()
        );
    }
}
