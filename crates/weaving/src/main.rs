use clap::{Parser, Subcommand};
use futures::future::join_all;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use owo_colors::OwoColorize;
use regex::Regex;
use resolve_path::PathResolveExt;
use rouille::websocket::{self, Message};
use routes::{serve_catchall, serve_websocket};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use template::{Templates, get_new_site};
use tokio::sync::{
    Mutex,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};
use weaver_lib::Weaver;

pub mod routes;
pub mod template;

type WsClients = Arc<Mutex<Vec<UnboundedSender<Message>>>>;

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
                _ => panic!("I don't know what template you asked for, is it spelled correctly?"),
            };

            get_new_site(template, output_path)
                .await
                .expect("failed to create your new site, sorry about that.");
        }
        Commands::Config { path, force } => {
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
watch_excludes = ["\\.git", "node_modules", "site", "\\.DS_Store"]
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
            let watch_excludes = instance.config.get_merged_watch_exclude_patterns();

            // Compile regexes once
            let compiled_excludes: Vec<Regex> = watch_excludes
                .iter()
                .filter_map(|pattern| match Regex::new(pattern) {
                    Ok(re) => Some(re),
                    Err(e) => {
                        eprintln!("Invalid regex pattern in config: '{}' - {}", pattern, e);
                        None
                    }
                })
                .collect();

            println!(
                "{}{}",
                "site available at http://".green(),
                &address.green()
            );

            let clients: WsClients = Arc::new(Mutex::new(Vec::new()));
            let clients_clone = clients.clone();
            let clients_broadcast = clients.clone();

            let (file_change_tx, mut file_change_rx): (
                UnboundedSender<String>,
                UnboundedReceiver<String>,
            ) = unbounded_channel();
            let file_change_tx_for_watcher = file_change_tx.clone();

            serve_tasks.push(tokio::spawn(async move {
                while let Some(message) = file_change_rx.recv().await {
                    let mut disconnected_clients = Vec::new();
                    let mut clients_lock = clients_broadcast.lock().await;

                    for (i, client_tx) in clients_lock.iter().enumerate() {
                        if let Err(err) = client_tx.send(websocket::Message::Text(message.clone()))
                        {
                            eprint!("ERROR sending reload: {}", err.red());
                            disconnected_clients.push(i);
                        } else {
                            continue;
                        };
                    }

                    for &i in disconnected_clients.iter().rev() {
                        clients_lock.remove(i);
                    }
                    println!(
                        "[WebSocket Broadcaster] Broadcasted '{}' to {} clients",
                        message.yellow(),
                        clients_lock.len()
                    );
                }
            }));

            let watch_path = safe_path.clone();
            let compiled_excludes_for_watcher = Arc::new(compiled_excludes);

            // Watch files for changes task
            serve_tasks.push(tokio::spawn(async move {
                let (tx, rx) = std::sync::mpsc::channel();
                let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
                watcher
                    .watch(path.as_ref(), RecursiveMode::Recursive)
                    .unwrap();
                println!("{}", "watching for changes.".blue());

                // Debouncing mechanism
                let mut last_build_time = tokio::time::Instant::now();
                let debounce_duration = Duration::from_millis(100);

                for res in rx {
                    let mut instance = Weaver::new(watch_path.clone());
                    match res {
                        Ok(e) => match e.kind {
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                                let mut should_rebuild = false;

                                for p in &e.paths {
                                    if !should_skip_path(p, &compiled_excludes_for_watcher) {
                                        println!("{:#?} changed, considering rebuild.", p.green());
                                        should_rebuild = true;
                                        break;
                                    }
                                }

                                if should_rebuild {
                                    let now = tokio::time::Instant::now();
                                    if now.duration_since(last_build_time) < debounce_duration {
                                        continue;
                                    }
                                    if !fs::exists(&e.paths[0]).unwrap() {
                                        println!(
                                            "{} was removed too quickly. Ignoring",
                                            &e.paths[0].display()
                                        );
                                        continue;
                                    }
                                    println!(
                                        "{:#?} changed ({:#?}), rebuilding.",
                                        &e.kind,
                                        e.paths.green()
                                    );
                                    let build_result = instance
                                        .scan_content()
                                        .scan_templates()
                                        .scan_partials()
                                        .build()
                                        .await;

                                    last_build_time = now;

                                    match build_result {
                                        Ok(_) => {
                                            println!("{}", "Built successfully".blue());
                                            if let Err(err) = file_change_tx_for_watcher
                                                .send("reload".to_string())
                                            {
                                                eprintln!("Error sending reload message: {}", err);
                                            }
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

            // We need to pass the current tokio handle down to the websocket handler.
            let tokio_runtime_handle = tokio::runtime::Handle::current();

            // HTTP server task (using tokio::spawn)
            serve_tasks.push(tokio::spawn(async move {
                let server_tokio_handle = tokio_runtime_handle.clone();
                rouille::start_server(address, move |request| {
                    let request_tokio_handle = server_tokio_handle.clone();

                    rouille::router!(request,
                        (GET) ["/ws"] => serve_websocket(request, clients_clone.clone(), request_tokio_handle),
                        _ => serve_catchall(&safe_path, request)
                    )
                });
            }));

            join_all(serve_tasks).await;
        }
    }

    Ok(())
}

fn should_skip_path(path: &Path, content_root: &PathBuf, excludes: &[Regex]) -> bool {
    let path_str = path.strip_prefix(content_root).unwrap().to_string_lossy();

    for re in excludes {
        if re.is_match(&path_str) {
            return true;
        }
    }

    false
}

fn sanitize_path(req_path: &str, with_root: bool) -> PathBuf {
    let mut sanitized = PathBuf::new();
    for component in Path::new(req_path).components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::ParentDir => {}
            Component::Normal(os_str) => sanitized.push(os_str),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }

    if with_root {
        format!("/{}", sanitized.display()).into()
    } else {
        sanitized
    }
}
