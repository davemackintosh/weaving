use std::sync::Arc;

use tokio::sync::Mutex;

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

    pub async fn render(&self, data: &liquid::model::Object) -> Result<String, BuildError> {
        match self {
            Self::LiquidBuilder {
                liquid_parser,
                weaver_template,
            } => {
                let wtemplate = weaver_template.lock().await;

                match liquid_parser
                    .parse(&wtemplate.contents)
                    .unwrap()
                    .render(&liquid::to_object(data).unwrap())
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
}

impl MarkdownRenderer {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        Self { document }
    }

    pub async fn render<D>(&self, _data: &D) -> Result<String, BuildError> {
        let doc_guard = self.document.lock().await;
        match markdown::to_html_with_options(doc_guard.markdown.as_str(), &markdown::Options::gfm())
        {
            Ok(html) => Ok(html),
            Err(err) => Err(BuildError::Err(err.to_string())),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::template::Template;

    use super::*;

    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_liquid() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/test_fixtures/liquid", base_path_wd);
        let template = Template::new_from_path(format!("{}/template.liquid", base_path).into());
        let renderer = TemplateRenderer::new(Arc::new(Mutex::new(template)));

        let data = liquid::object!({
            "page": {
            "title": "hello"
        }
        });

        assert_eq!(
            "<!doctype html>
<html>
	<head>
		<title>hello</title>
	</head>
	<body></body>
</html>
",
            renderer.render(&data).await.unwrap()
        );
    }
}
