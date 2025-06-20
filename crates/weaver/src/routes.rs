use std::path::{Path, PathBuf};

pub fn route_from_path(content_dir: PathBuf, path: PathBuf) -> String {
    // 1. Strip the base content directory prefix
    let relative_path = match path.strip_prefix(&content_dir) {
        Ok(p) => p,
        Err(_) => {
            // This should ideally not happen if paths are correctly managed.
            // Or it means the path is outside the content directory.
            // Handle this error case appropriately, e.g., panic, return an error, or log.
            // For now, let's just panic and halt the build because this won't output something
            // visible or usable to anyone.
            println!(
                "Warning: Path {:?} is not within content directory {:?}",
                path, content_dir
            );
            return "".to_string();
        }
    };

    let mut route_parts: Vec<String> = relative_path
        .components()
        .filter_map(|c| {
            // Filter out relative path components, and root/prefix components
            match c {
                std::path::Component::Normal(os_str) => Some(os_str.to_string_lossy().into_owned()),
                _ => None,
            }
        })
        .collect();

    // 2. Handle file extension and "pretty URLs"
    if let Some(last_segment) = route_parts.pop() {
        let original_filename_path = Path::new(&last_segment);

        if original_filename_path.file_stem().is_some() {
            let stem = original_filename_path
                .file_stem()
                .unwrap()
                .to_string_lossy();

            if stem == "index" {
                // If it's an index file, the URI is just its parent directory
                // The parent directory is already represented by the remaining route_parts
                // So, no need to add "index" to the route.
                // Example: content/posts/index.md -> /posts/
            } else {
                // For other files, use the stem as the segment and add a trailing slash
                // Example: content/posts/my-post.md -> /posts/my-post/
                route_parts.push(stem.into_owned());
            }
        }
    }

    // 3. Join parts with forward slashes and ensure leading/trailing slashes
    let mut route = format!("/{}", route_parts.join("/"));

    // Ensure trailing slash for directories, unless it's the root '/'
    if route.len() > 1 {
        route.push('/');
    }

    // Special case for root index.md (e.g., content/index.md -> /)
    // If the original relative_path was just "index.md"
    if relative_path.to_string_lossy() == "index.md" {
        route = "/".to_string();
    }

    route
}

#[cfg(test)]
mod test {
    use crate::Weaver;

    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn test_route_from_path() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/config", base_path_wd);
        let inst = Weaver::new(format!("{}/custom_config", base_path).into());

        assert_eq!(
            "/blog/post1/",
            route_from_path(
                inst.config.content_dir.clone().into(),
                format!("{}/blog/post1.md", inst.config.content_dir).into()
            )
        );
    }

    #[test]
    #[should_panic]
    fn test_content_out_of_path() {
        let base_path_wd = std::env::current_dir().unwrap().display().to_string();
        let base_path = format!("{}/test_fixtures/config", base_path_wd);
        let inst = Weaver::new(format!("{}/custom_config", base_path).into());
        route_from_path(
            inst.config.content_dir.clone().into(),
            "madeup/blog/post1.md".into(),
        );
    }
}
