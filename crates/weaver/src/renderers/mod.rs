use std::{ffi::OsStr, path::PathBuf};

use liquid::ObjectView;

use crate::BuildError;

pub enum TemplateRenderer {
    LiquidBuilder {
        liquid_parser: liquid::Parser,
        weaver_template: crate::Template,
    },
}

impl TemplateRenderer {
    pub fn new(path: PathBuf) -> Self {
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

    pub fn render<D: ObjectView>(&self, data: &D) -> Result<String, BuildError> {
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
