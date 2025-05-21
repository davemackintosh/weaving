use clap::{Parser, Subcommand};
use futures::future::join_all;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use owo_colors::OwoColorize;
use resolve_path::PathResolveExt;
use rouille::Response;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use template::{Templates, get_new_site};
use weaver_lib::Weaver;

pub mod template;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    Build {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
    New {
        #[arg(short, long, default_value = "my-site")]
        name: String,

        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        #[arg(short, long, default_value = "default")]
        template: String,
    },
    Config {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        #[arg(short, long, default_value = "false")]
        force: bool,
    },
    Serve {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.cmd {
        Commands::Build { path } => {
            let mut instance = Weaver::new(fs::canonicalize(path.resolve())?);

            instance
                .scan_content()
                .scan_templates()
                .scan_partials()
                .build()
                .await?;
        }
        Commands::New {
            path,
            name,
            template,
        } => {
            let target_path = fs::canonicalize(path.resolve())?;
            let output_path: PathBuf = format!("{}/{}", target_path.display(), name).into();
            let template = match template.as_str() {
                "default" => Templates::Default,
                _ => panic!("I don't know what template you asked for, is it spelt correctly?"),
            };

            get_new_site(template, output_path)
                .await
                .expect("failed to create your new site, sorry about that.");
        }
        Commands::Config { path, force } => {
            // Check if there is a config file or not and then check the force flag.
            let target_path = fs::canonicalize(path.resolve())?;
            let config_exists =
                fs::exists(format!("{}/weaving.toml", &target_path.display())).unwrap();

            if !config_exists || force {
                fs::write(
                    format!("{}/weaving.toml", &target_path.display()),
                    r#"version = 1
content_dir = "content"
base_url = "localhost:8080"
partials_dir = "partials"
public_dir = "public"
build_dir = "site"
template_dir = "templates"
templating_language = "liquid"

[image_config]
quality = 83

[serve_config]
watch_excludes = [".git", "node_modules", "site"]
npm_build = false
address = "localhost:8080"
"#,
                )?;
            }
        }
        Commands::Serve { path } => {
            let safe_path = fs::canonicalize(path.resolve())?;
            let mut serve_tasks = vec![];

            println!("{}", "building".green());
            let mut instance = Weaver::new(fs::canonicalize(path.resolve())?);
            instance
                .scan_content()
                .scan_templates()
                .scan_partials()
                .build()
                .await?;

            let address = instance.config.serve_config.address.clone();

            println!(
                "{}{}",
                "site available at http://".green(),
                &address.green()
            );

            let watch_path = safe_path.clone();

            // Watch files for changes.
            serve_tasks.push(tokio::spawn(async move {
                let (tx, rx) = std::sync::mpsc::channel();
                let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
                watcher
                    .watch(path.as_ref(), RecursiveMode::Recursive)
                    .unwrap();
                println!("{}", "watching for changes.".blue());

                for res in rx {
                    let mut instance = Weaver::new(watch_path.clone());
                    match res {
                        Ok(e) => match e.kind {
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                                // Should probably add excludes to config to ignore things like
                                // node_modules but for now, ignore the build dir and git.
                                let skip_build = e.paths.iter().any(|p| {
                                    p.starts_with(instance.config.build_dir.clone())
                                        || p.ends_with("~")
                                        || p.components().any(|c| {
                                            if let std::path::Component::Normal(os_str) = c {
                                                os_str == ".git"
                                            } else {
                                                false // Only check Normal components
                                            }
                                        })
                                });

                                if !skip_build {
                                    println!("{:#?} changed, rebuilding.", e.paths.green());
                                    let build_result = instance
                                        .scan_content()
                                        .scan_templates()
                                        .scan_partials()
                                        .build()
                                        .await;

                                    match build_result {
                                        Ok(_) => {
                                            println!("{}", "Built successfully".blue());
                                        }
                                        Err(err) => {
                                            eprintln!(
                                                "{} {}",
                                                "Failed to build because".red(),
                                                err.to_string().red()
                                            );
                                        }
                                    }
                                }
                            }
                            _ => {}
                        },
                        Err(error) => eprintln!("Error: {error:?}"),
                    }
                }
            }));

            // HTTP server task.
            serve_tasks.push(tokio::spawn(async move {
                rouille::start_server(address, move |request| {
                    let req_path = request.url();
                    let instance = Weaver::new(safe_path.clone());
                    println!(
                        "Received {} request for: {}",
                        request.method().blue(),
                        req_path.yellow()
                    );

                    let sanitized_req_path = sanitize_path(&req_path);

                    let mut file_path = format!(
                        "{}/{}",
                        instance.config.build_dir,
                        &sanitized_req_path.display()
                    );

                    // If the request is for a directory (ends with / or is the root /)
                    // append index.html
                    if req_path.ends_with('/') || req_path == "/" {
                        file_path = format!("{}/index.html", file_path);
                    }

                    println!("Serving: {:?}", &file_path.green());

                    match fs::read(&file_path) {
                        Ok(content) => {
                            let mime_type =
                                mime_guess::from_path(&file_path).first_or_octet_stream();

                            Response::from_data(mime_type.to_string(), content)
                        }
                        Err(err) => {
                            // Handle file system errors
                            let status = match err.kind() {
                                io::ErrorKind::NotFound => 404,
                                _ => 500,
                            };

                            eprintln!("Error reading file {:?}: {}", file_path.yellow(), err.red());
                            Response::text(format!("Error: {}", err)).with_status_code(status)
                        }
                    }
                });
            }));

            join_all(serve_tasks).await;
        }
    }

    Ok(())
}

// Helper function to sanitize the requested path to prevent directory traversal
// This function remains the same as it's file-system path sanitization logic.
fn sanitize_path(req_path: &str) -> PathBuf {
    // Start with an empty path
    let mut sanitized = PathBuf::new();

    // Iterate over path components
    for component in Path::new(req_path).components() {
        use std::path::Component;
        match component {
            // Ignore "." components
            Component::CurDir => {}
            // Ignore ".." components to prevent moving up directories
            Component::ParentDir => {}
            // Add normal directory or file names
            Component::Normal(os_str) => sanitized.push(os_str),
            // Ignore root or prefix components
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    sanitized
}
