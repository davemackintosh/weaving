use std::{ffi::OsStr, path::PathBuf};

use liquid::ObjectView;

use crate::{BuildError, document::Document};

pub trait Renderer {
    fn new(path: PathBuf) -> Self;
    fn render<D: ObjectView>(&self, data: &D) -> Result<String, BuildError>;
}

pub enum TemplateRenderer {
    LiquidBuilder {
        liquid_parser: liquid::Parser,
        weaver_template: crate::Template,
    },
}

impl Renderer for TemplateRenderer {
    fn new(path: PathBuf) -> Self {
        match path.extension().and_then(OsStr::to_str).unwrap() {
            "liquid" => Self::LiquidBuilder {
                liquid_parser: liquid::ParserBuilder::with_stdlib().build().unwrap(),
                weaver_template: crate::Template::new_from_path(path),
            },
            _ => panic!(
                "Not sure what templating engine to use for this file. {}",
                path.display()
            ),
        }
    }

    fn render<D: ObjectView>(&self, data: &D) -> Result<String, BuildError> {
        match self {
            Self::LiquidBuilder {
                liquid_parser,
                weaver_template,
            } => match liquid_parser
                .parse(&weaver_template.contents)
                .unwrap()
                .render(data)
            {
                Ok(result) => Ok(result),
                Err(err) => Err(BuildError::Err(err.to_string())),
            },
        }
    }
}

pub struct MarkdownRenderer {
    document: Document,
}

impl Renderer for MarkdownRenderer {
    fn new(path: PathBuf) -> Self {
        Self {
            document: Document::new_from_path(path),
        }
    }

    fn render<D: ObjectView>(&self, data: &D) -> Result<String, BuildError> {
        let tree = markdown::to_mdast(
            &self.document.markdown,
            &markdown::ParseOptions {
                constructs: markdown::Constructs {
                    frontmatter: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();

        let frontmatter = tree.children().filter(|n| match n {
            markdown::mdast::Node::Yaml(yml) => true,
            _ => false,
        });
        let markdown = tree.children().iter().filter(|n| match n {
            markdown::mdast::Node::Yaml(yml) => false,
            _ => true,
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn test_liquid() {
        let base_path_wd = std::env::current_dir()
            .unwrap()
            .as_os_str()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let base_path = format!("{}/test_fixtures/liquid", base_path_wd);
        let renderer = TemplateRenderer::new(format!("{}/template.liquid", base_path).into());

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
            renderer.render(&data).unwrap()
        );
    }
}
