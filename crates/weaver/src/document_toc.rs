use markdown::{ParseOptions, mdast::Node};
use slug::slugify;

use crate::document::Heading;

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
                    text.push_str(&extract_text_from_mdast_inline(child)); // Recurse
                }
            }
        }
        _ => {
            // For other node types, if they have children, recurse into them
            if let Some(children) = node.children() {
                for child in children.iter() {
                    text.push_str(&extract_text_from_mdast_inline(child));
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
                text.push_str(&extract_text_from_mdast_inline(child));
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
            collect_mdast_headings_to_map(child, headings_map);
        }
    }
}

pub fn toc_from_document(markdown: &str) -> Vec<Heading> {
    let mut toc_map = vec![];
    let ast = markdown::to_mdast(markdown, &ParseOptions::gfm()).unwrap();
    collect_mdast_headings_to_map(&ast, &mut toc_map);
    toc_map
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::sync::Mutex;

    use crate::document::Document;

    use super::*;

    #[tokio::test]
    async fn test_markdown_toc_generation() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/markdown", base_path_wd);
        let doc_arc = Arc::new(Mutex::new(Document::new_from_path(
            format!("{}/with_headings.md", base_path).into(),
        )));

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
            toc_from_document(doc_arc.lock().await.markdown.as_str())
        );
    }
}
