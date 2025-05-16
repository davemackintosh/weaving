use std::{
    fs,
    io::{self, ErrorKind},
    path::PathBuf,
    process::Command,
};

use tempfile::tempdir;
use walkdir::WalkDir;

pub struct TemplateAt {
    pub path: PathBuf,
    pub content: String,
}

pub enum Templates {
    Default,
}

fn copy_dir_contents(src: &PathBuf, dest: &PathBuf) -> Result<(), io::Error> {
    if !src.is_dir() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Source is not a directory",
        ));
    }
    if dest.exists() && dest.read_dir()?.next().is_some() {
        return Err(io::Error::new(
            ErrorKind::AlreadyExists,
            format!(
                "Destination directory already exists and is not empty: {}",
                dest.display()
            ),
        ));
    }

    fs::create_dir_all(dest)?;

    for entry in WalkDir::new(src).min_depth(1) {
        let entry = entry?;
        let path = entry.path();

        let relative_path = path.strip_prefix(src).unwrap();

        let dest_path = dest.join(relative_path);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else if entry.file_type().is_file() {
            fs::copy(path, &dest_path)?;
        }
    }
    Ok(())
}

pub async fn get_new_site(
    template: Templates,
    output_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let template_repo_url = match template {
        Templates::Default => "https://github.com/davemackintosh/weaving-default-site",
    };
    println!(
        "Creating new project from {} at {}...",
        template_repo_url,
        output_path.display()
    );

    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path().to_path_buf();

    println!(
        "Cloning {} into temporary directory: {}...",
        template_repo_url,
        temp_path.display()
    );

    let mut command = Command::new("git");
    command.arg("clone");
    command.arg("--depth").arg("1");
    command.arg(template_repo_url);
    command.arg(&temp_path);

    let status = command.status()?;

    if !status.success() {
        eprintln!("Error: Git clone failed with exit status: {}", status);
        eprintln!("Please ensure Git is installed and you have network access.");
        return Err(format!("Git clone failed for URL: {}", template_repo_url).into());
    }

    println!("Clone successful.");

    println!("Copying cloned files to {}...", output_path.display());
    copy_dir_contents(&temp_path, &output_path)?;

    println!("Project created successfully at {}.", output_path.display());

    Ok(())
}
