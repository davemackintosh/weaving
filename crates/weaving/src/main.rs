use clap::{Parser, Subcommand};
use crossbeam_channel::{Receiver, Sender, unbounded};
use futures::future::join_all;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use owo_colors::OwoColorize;
use resolve_path::PathResolveExt;
use rouille::websocket::{self, Message};
use routes::{serve_catchall, serve_websocket};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use template::{Templates, get_new_site};
use tokio::sync::Mutex;
use weaver_lib::Weaver;

pub mod routes;
pub mod template;

type WsClients = Arc<Mutex<Vec<Sender<Message>>>>;

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

            let clients: WsClients = Arc::new(Mutex::new(Vec::new()));
            let clients_clone = clients.clone(); // For HTTP server thread
            let clients_broadcast = clients.clone(); // For broadcasting thread

            let (file_change_tx, file_change_rx): (Sender<String>, Receiver<String>) = unbounded();
            let file_change_tx_for_watcher = file_change_tx.clone(); // For watcher thread

            serve_tasks.push(tokio::spawn(async move {
                for message in file_change_rx {
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
            // --- End WebSocket setup ---

            let watch_path = safe_path.clone();

            // Watch files for changes task
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
                                let skip_build = e.paths.iter().any(|p| {
                                    p.starts_with(&instance.config.build_dir)
                                        || p.ends_with("~")
                                        || p.components().any(|c| {
                                            if let std::path::Component::Normal(os_str) = c {
                                                instance
                                                    .config
                                                    .serve_config
                                                    .watch_excludes
                                                    .iter()
                                                    .any(|exclude| {
                                                        os_str.to_str().unwrap() == exclude.as_str()
                                                    })
                                            } else {
                                                false
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
                                        .await; // This await needs tokio runtime

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
