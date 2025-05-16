use std::{
    fs, io,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use resolve_path::PathResolveExt;
use rouille::Response;
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
    Serve {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        #[arg(short, long, default_value = "localhost:8080")]
        address: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.cmd {
        Commands::Build { path } => {
            let mut instance = Weaver::new(fs::canonicalize(path.resolve())?);

            instance.scan_content().scan_templates().build().await?;
        }
        Commands::New {
            path,
            name,
            template,
        } => {
            println!("{}", path.resolve().display());
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
        Commands::Serve { path, address } => {
            let instance = Weaver::new(fs::canonicalize(path.resolve())?);
            dbg!("path is {}", fs::canonicalize(path.resolve())?);
            dbg!("instance build path is {}", &instance.config.build_dir);
            rouille::start_server(address, move |request| {
                let req_path = request.url();
                dbg!(format!("Received request for: {}", req_path));

                let sanitized_req_path = sanitize_path(&req_path);
                dbg!("sanitzed to {}", &sanitized_req_path);

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

                dbg!(format!("Attempting to serve file: {:?}", file_path));

                // Read the file content synchronously using std::fs
                match fs::read(&file_path) {
                    Ok(content) => {
                        let mime_type = mime_guess::from_path(&file_path).first_or_octet_stream();

                        Response::from_data(mime_type.to_string(), content)
                    }
                    Err(err) => {
                        // Handle file system errors
                        let status = match err.kind() {
                            io::ErrorKind::NotFound => 404,
                            _ => 500,
                        };

                        eprintln!("Error reading file {:?}: {}", file_path, err);

                        // Create an error response
                        Response::text(format!("Error: {}", err)).with_status_code(status) // Set the appropriate status code
                    }
                }
            });
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
