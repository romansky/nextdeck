use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

const APP_PACKAGE: &str = "cargo-test-tui";

#[derive(Debug, Parser)]
#[command(version, about = "Local project automation")]
struct Cli {
    #[command(subcommand)]
    command: XtaskCommand,
}

#[derive(Debug, Subcommand)]
enum XtaskCommand {
    #[command(about = "Run local checks expected before publishing")]
    Check {
        #[arg(long, help = "Allow cargo package to run with a dirty worktree")]
        allow_dirty: bool,
    },
    #[command(about = "Create a local .crate package in target/package")]
    Package {
        #[arg(long, help = "Allow packaging with a dirty worktree")]
        allow_dirty: bool,
    },
    #[command(about = "Install the app locally from the current workspace")]
    InstallPath,
    #[command(about = "Install the app locally from the generated .crate package")]
    InstallPackage {
        #[arg(long, help = "Allow packaging with a dirty worktree first")]
        allow_dirty: bool,
    },
    #[command(about = "Package and install the verified package locally")]
    PublishLocal {
        #[arg(long, help = "Allow packaging with a dirty worktree")]
        allow_dirty: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace = workspace_root()?;

    match cli.command {
        XtaskCommand::Check { allow_dirty } => check(&workspace, allow_dirty),
        XtaskCommand::Package { allow_dirty } => {
            let artifact = package(&workspace, allow_dirty)?;
            println!("Packaged {}", artifact.crate_path.display());
            Ok(())
        }
        XtaskCommand::InstallPath => install_path(&workspace),
        XtaskCommand::InstallPackage { allow_dirty } => {
            let artifact = package(&workspace, allow_dirty)?;
            install_crate(&workspace, &artifact.unpacked_dir)
        }
        XtaskCommand::PublishLocal { allow_dirty } => {
            let artifact = package(&workspace, allow_dirty)?;
            install_crate(&workspace, &artifact.unpacked_dir)
        }
    }
}

fn check(workspace: &Path, allow_dirty: bool) -> Result<()> {
    run(workspace, "cargo", ["fmt", "--all", "--check"])?;
    run(
        workspace,
        "cargo",
        ["test", "--workspace", "--exclude", "xtask"],
    )?;
    package(workspace, allow_dirty)?;
    Ok(())
}

#[derive(Debug)]
struct PackageArtifact {
    crate_path: PathBuf,
    unpacked_dir: PathBuf,
}

fn package(workspace: &Path, allow_dirty: bool) -> Result<PackageArtifact> {
    let version = package_version(workspace)?;
    let crate_path = workspace
        .join("target")
        .join("package")
        .join(format!("{APP_PACKAGE}-{version}.crate"));
    let unpacked_dir = workspace
        .join("target")
        .join("package")
        .join(format!("{APP_PACKAGE}-{version}"));

    let package_verify_target = workspace.join("target/package-verify");
    let package_verify_target = package_verify_target
        .to_str()
        .context("package verify target path is not UTF-8")?
        .to_owned();
    let mut args = vec![
        "package".to_owned(),
        "-p".to_owned(),
        APP_PACKAGE.to_owned(),
        "--locked".to_owned(),
        "--target-dir".to_owned(),
        package_verify_target,
    ];
    if allow_dirty {
        args.push("--allow-dirty".to_owned());
    }
    run(workspace, "cargo", args)?;

    if !crate_path.exists() {
        bail!("expected package artifact at {}", crate_path.display());
    }
    if !unpacked_dir.exists() {
        bail!(
            "expected verified package directory at {}",
            unpacked_dir.display()
        );
    }
    Ok(PackageArtifact {
        crate_path,
        unpacked_dir,
    })
}

fn install_path(workspace: &Path) -> Result<()> {
    run(
        workspace,
        "cargo",
        ["install", "--path", ".", "--locked", "--force"],
    )
}

fn install_crate(workspace: &Path, crate_path: &Path) -> Result<()> {
    let crate_arg = crate_path
        .to_str()
        .with_context(|| format!("package path is not UTF-8: {}", crate_path.display()))?;
    run(
        workspace,
        "cargo",
        ["install", "--path", crate_arg, "--locked", "--force"],
    )
}

fn package_version(workspace: &Path) -> Result<String> {
    let cargo_toml = fs::read_to_string(workspace.join("Cargo.toml"))
        .context("reading root Cargo.toml for package version")?;
    let mut in_package = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        match trimmed {
            "[package]" => in_package = true,
            "[workspace]" | "[dependencies]" => in_package = false,
            _ => {}
        }

        if in_package {
            if let Some(version) = trimmed.strip_prefix("version = ") {
                return Ok(version.trim_matches('"').to_owned());
            }
        }
    }

    bail!("could not find [package] version in root Cargo.toml")
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest directory has no parent")
}

fn run<I, S>(cwd: &Path, program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    print_command(program, &args);
    let status = Command::new(program)
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("running {program}"))?;

    if !status.success() {
        bail!("{program} exited with {status}");
    }
    Ok(())
}

fn print_command(program: &str, args: &[OsString]) {
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    println!("$ {program} {rendered}");
}
