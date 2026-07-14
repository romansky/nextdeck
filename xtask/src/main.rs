use std::{
    collections::BTreeSet,
    env,
    ffi::OsString,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use flate2::{Compression, write::GzEncoder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tar::Builder as TarBuilder;

const TUI_PACKAGE: &str = "nextdeck";
const TUI_BINARY: &str = "nextdeck";
const HELPER_PACKAGE: &str = "nextdeck-helper";
const DIST_DIR: &str = "target/dist";
const LOCAL_PUBLISH_DIR: &str = "target/local-publish";

#[derive(Debug, Parser)]
#[command(version, about = "Local project automation")]
struct Cli {
    #[command(subcommand)]
    command: XtaskCommand,
}

#[derive(Debug, Subcommand)]
enum XtaskCommand {
    #[command(about = "Run local TUI checks expected before publishing")]
    TuiCheck {
        #[arg(long, help = "Allow cargo package to run with a dirty worktree")]
        allow_dirty: bool,
    },
    #[command(about = "Install the TUI package locally from the workspace")]
    TuiPublishLocal,
    #[command(about = "Build, archive, checksum, and sign a TUI release artifact")]
    TuiRelease {
        #[arg(
            long,
            help = "Release version. Defaults to the root Cargo.toml version"
        )]
        version: Option<String>,
        #[arg(long, help = "Rust target triple. Defaults to the host target")]
        target: Option<String>,
        #[arg(long, help = "Allow release artifacts with a dirty worktree")]
        allow_dirty: bool,
        #[arg(long, help = "Create artifacts without cosign signatures")]
        skip_sign: bool,
        #[arg(
            long,
            help = "GitHub repository as owner/repo, used to generate a Homebrew formula"
        )]
        github_repo: Option<String>,
    },
    #[command(about = "Generate a Homebrew formula from TUI release artifact checksums")]
    TuiHomebrewFormula {
        #[arg(
            long,
            help = "Formula version. Defaults to the root Cargo.toml version"
        )]
        version: Option<String>,
        #[arg(
            long,
            env = "GITHUB_REPOSITORY",
            help = "GitHub repository as owner/repo"
        )]
        github_repo: String,
        #[arg(long, default_value = DIST_DIR, help = "Directory containing *.tar.gz.sha256 files")]
        dist_dir: PathBuf,
        #[arg(long, help = "Output formula path")]
        output: PathBuf,
    },
    #[command(about = "Run local checks for the nextdeck-helper crate")]
    HelperCheck {
        #[arg(long, help = "Allow cargo package to run with a dirty worktree")]
        allow_dirty: bool,
    },
    #[command(about = "Create a verified nextdeck-helper package")]
    HelperPackage {
        #[arg(long, help = "Allow packaging with a dirty worktree")]
        allow_dirty: bool,
    },
    #[command(about = "Publish nextdeck-helper locally and smoke-test it from another project")]
    HelperPublishLocal {
        #[arg(long, help = "Allow packaging with a dirty worktree")]
        allow_dirty: bool,
    },
    #[command(about = "Run crates.io publish wiring for nextdeck-helper")]
    HelperPush {
        #[arg(long, help = "Allow publishing with a dirty worktree")]
        allow_dirty: bool,
        #[arg(
            long,
            help = "Actually publish instead of running cargo publish --dry-run"
        )]
        execute: bool,
    },
    #[command(about = "Run a tests-tree path and print Nextdeck-style output as JSONL")]
    ReproNextdeckRun {
        #[arg(
            long,
            help = "Tests tree path, for example workspace or nextdeck::output::tests::case"
        )]
        path: String,
    },
}

fn main() -> Result<()> {
    nextdeck_helper::xtask_clap_info!(Cli);

    let cli = Cli::parse();
    let workspace = workspace_root()?;

    match cli.command {
        XtaskCommand::TuiCheck { allow_dirty } => tui_check(&workspace, allow_dirty),
        XtaskCommand::TuiPublishLocal => install_tui_workspace(&workspace),
        XtaskCommand::TuiRelease {
            version,
            target,
            allow_dirty,
            skip_sign,
            github_repo,
        } => tui_release(
            &workspace,
            version,
            target,
            allow_dirty,
            skip_sign,
            github_repo,
        ),
        XtaskCommand::TuiHomebrewFormula {
            version,
            github_repo,
            dist_dir,
            output,
        } => tui_homebrew_formula(&workspace, version, &github_repo, &dist_dir, &output),
        XtaskCommand::HelperCheck { allow_dirty } => helper_check(&workspace, allow_dirty),
        XtaskCommand::HelperPackage { allow_dirty } => {
            let artifact = package_crate(&workspace, HELPER_PACKAGE, allow_dirty)?;
            println!("Packaged {}", artifact.crate_path.display());
            Ok(())
        }
        XtaskCommand::HelperPublishLocal { allow_dirty } => {
            helper_publish_local(&workspace, allow_dirty)
        }
        XtaskCommand::HelperPush {
            allow_dirty,
            execute,
        } => helper_push(&workspace, allow_dirty, execute),
        XtaskCommand::ReproNextdeckRun { path } => repro_nextdeck_run(&workspace, &path),
    }
}

fn tui_check(workspace: &Path, allow_dirty: bool) -> Result<()> {
    run(workspace, "cargo", ["fmt", "--all", "--check"])?;
    run(
        workspace,
        "cargo",
        [
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run(
        workspace,
        "cargo",
        ["test", "--workspace", "--all-features"],
    )?;
    check_package_contents(workspace, TUI_PACKAGE, allow_dirty)?;
    Ok(())
}

fn helper_check(workspace: &Path, allow_dirty: bool) -> Result<()> {
    run(workspace, "cargo", ["fmt", "--all", "--check"])?;
    run(
        workspace,
        "cargo",
        [
            "clippy",
            "-p",
            HELPER_PACKAGE,
            "--all-targets",
            "--features",
            "xtask-clap",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run(
        workspace,
        "cargo",
        ["test", "-p", HELPER_PACKAGE, "--features", "xtask-clap"],
    )?;
    package_crate(workspace, HELPER_PACKAGE, allow_dirty)?;
    Ok(())
}

#[derive(Debug)]
struct PackageArtifact {
    package: String,
    version: String,
    crate_path: PathBuf,
    unpacked_dir: PathBuf,
}

fn package_crate(workspace: &Path, package: &str, allow_dirty: bool) -> Result<PackageArtifact> {
    let version = package_version(workspace, package)?;
    let package_verify_target = workspace.join("target/package-verify").join(package);
    let package_dir = package_verify_target.join("package");
    let crate_path = package_dir.join(format!("{package}-{version}.crate"));
    let unpacked_dir = package_dir.join(format!("{package}-{version}"));
    let package_verify_target = package_verify_target
        .to_str()
        .context("package verify target path is not UTF-8")?
        .to_owned();
    let mut args = vec![
        "package".to_owned(),
        "-p".to_owned(),
        package.to_owned(),
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
        package: package.to_owned(),
        version,
        crate_path,
        unpacked_dir,
    })
}

fn check_package_contents(workspace: &Path, package: &str, allow_dirty: bool) -> Result<()> {
    let mut args = vec![
        "package".to_owned(),
        "-p".to_owned(),
        package.to_owned(),
        "--locked".to_owned(),
        "--list".to_owned(),
    ];
    if allow_dirty {
        args.push("--allow-dirty".to_owned());
    }
    run(workspace, "cargo", args)
}

fn install_tui_workspace(workspace: &Path) -> Result<()> {
    println!(
        "Installing {TUI_PACKAGE} from workspace path because {HELPER_PACKAGE} is not published yet."
    );
    run(
        workspace,
        "cargo",
        ["install", "--path", ".", "--locked", "--force"],
    )?;
    verify_local_install()
}

fn helper_publish_local(workspace: &Path, allow_dirty: bool) -> Result<()> {
    let artifact = package_crate(workspace, HELPER_PACKAGE, allow_dirty)?;
    let local_dir = workspace
        .join(LOCAL_PUBLISH_DIR)
        .join(format!("{}-{}", artifact.package, artifact.version));
    if local_dir.exists() {
        fs::remove_dir_all(&local_dir)
            .with_context(|| format!("removing old local package {}", local_dir.display()))?;
    }
    copy_dir(&artifact.unpacked_dir, &local_dir)?;
    smoke_test_local_lib(workspace, &local_dir)?;
    println!("Packaged {}", artifact.crate_path.display());
    println!("Local package {}", local_dir.display());
    println!(
        "Use from another project with: {} = {{ path = \"{}\" }}",
        HELPER_PACKAGE,
        toml_escape_path(&local_dir)
    );
    Ok(())
}

fn helper_push(workspace: &Path, allow_dirty: bool, execute: bool) -> Result<()> {
    if !execute {
        println!("Dry-running crates.io publish; pass --execute to publish for real.");
    }

    let mut args = vec![
        "publish".to_owned(),
        "-p".to_owned(),
        HELPER_PACKAGE.to_owned(),
        "--locked".to_owned(),
    ];
    if allow_dirty {
        args.push("--allow-dirty".to_owned());
    }
    if !execute {
        args.push("--dry-run".to_owned());
    }
    run(workspace, "cargo", args)
}

fn repro_nextdeck_run(workspace: &Path, tree_path: &str) -> Result<()> {
    let tree_path = tree_path.trim();
    if tree_path.is_empty() {
        bail!("--path must not be empty");
    }
    let mut args = vec![
        "nextest".to_owned(),
        "run".to_owned(),
        "--message-format".to_owned(),
        "libtest-json-plus".to_owned(),
        "--message-format-version".to_owned(),
        "0.1".to_owned(),
        "--show-progress".to_owned(),
        "none".to_owned(),
        "--status-level".to_owned(),
        "none".to_owned(),
        "--final-status-level".to_owned(),
        "none".to_owned(),
        "--success-output".to_owned(),
        "immediate".to_owned(),
        "--no-input-handler".to_owned(),
    ];
    args.extend(nextest_args_for_tree_path(workspace, tree_path)?);

    let command_line = format!("cargo {}", args.join(" "));
    let mut command = Command::new("cargo");
    command
        .args(&args)
        .current_dir(workspace)
        .env("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1")
        .env(nextdeck_helper::ENV_VAR, nextdeck_helper::ENV_VALUE)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    print_jsonl(json!({
        "kind": "run",
        "event": "started",
        "cwd": workspace.display().to_string(),
        "tree_path": tree_path,
        "command": command_line,
    }))?;

    let output = command
        .output()
        .with_context(|| format!("running {command_line}"))?;
    print_stream_as_jsonl("stdout", &String::from_utf8_lossy(&output.stdout))?;
    print_stream_as_jsonl("stderr", &String::from_utf8_lossy(&output.stderr))?;
    print_jsonl(json!({
        "kind": "run",
        "event": "finished",
        "status": output.status.to_string(),
        "success": output.status.success(),
        "code": output.status.code(),
    }))?;

    Ok(())
}

fn smoke_test_local_lib(workspace: &Path, local_crate: &Path) -> Result<()> {
    let project_dir = workspace
        .join(LOCAL_PUBLISH_DIR)
        .join(format!("{HELPER_PACKAGE}-smoke"));
    if project_dir.exists() {
        fs::remove_dir_all(&project_dir)
            .with_context(|| format!("removing old smoke project {}", project_dir.display()))?;
    }
    fs::create_dir_all(project_dir.join("src"))
        .with_context(|| format!("creating smoke project {}", project_dir.display()))?;

    fs::write(
        project_dir.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"nextdeck-helper-smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{} = {{ path = \"{}\", features = [\"xtask-clap\"] }}\nclap = {{ version = \"4.5\", features = [\"derive\"] }}\nserde_json = \"1.0\"\n",
            HELPER_PACKAGE,
            toml_escape_path(local_crate)
        ),
    )
    .with_context(|| format!("writing {}", project_dir.join("Cargo.toml").display()))?;
    fs::write(
        project_dir.join("src/lib.rs"),
        r#"use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Run checks")]
    Check {
        #[arg(long)]
        allow_dirty: bool,
    },
}

#[test]
fn emits_events_and_xtask_metadata() {
    std::env::set_var(
        nextdeck_helper::ENV_VAR,
        nextdeck_helper::ENV_VALUE,
    );
    assert!(nextdeck_helper::enabled());
    nextdeck_helper::event!("smoke"; "kind" => "local");
    std::env::remove_var(nextdeck_helper::ENV_VAR);

    let mut metadata = Vec::new();
    let handled = nextdeck_helper::xtask::handle_nextdeck_info_from::<Cli, _, _, _>(
        ["xtask", "nextdeck-info", "--format", "json"],
        &mut metadata,
    )
    .expect("Nextdeck metadata");
    assert!(handled);
    let metadata = String::from_utf8(metadata).expect("utf8");
    assert!(metadata.contains("\"name\": \"check\""));
    assert!(metadata.contains("\"long\": \"allow-dirty\""));
}
"#,
    )
    .with_context(|| format!("writing {}", project_dir.join("src/lib.rs").display()))?;

    run(&project_dir, "cargo", ["test", "--quiet"])
}

fn toml_escape_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("creating directory {}", to.display()))?;
    for entry in
        fs::read_dir(from).with_context(|| format!("reading directory {}", from.display()))?
    {
        let entry = entry.with_context(|| format!("reading entry in {}", from.display()))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("reading file type for {}", source.display()))?;

        if file_type.is_dir() {
            copy_dir(&source, &target)?;
        } else if file_type.is_file() {
            fs::copy(&source, &target)
                .with_context(|| format!("copying {} to {}", source.display(), target.display()))?;
        }
    }
    Ok(())
}

fn verify_local_install() -> Result<()> {
    let installed = cargo_install_bin_path()?;
    if !installed.exists() {
        bail!(
            "cargo install completed but expected binary was not found at {}",
            installed.display()
        );
    }

    let path_binary = find_on_path(TUI_BINARY);
    let installed = fs::canonicalize(&installed)
        .with_context(|| format!("canonicalizing installed binary {}", installed.display()))?;
    let version = command_stdout(&installed, ["--version"])?;

    println!("Installed {} at {}", version.trim(), installed.display());

    match path_binary {
        Some(path_binary) => {
            let path_binary = fs::canonicalize(&path_binary)
                .with_context(|| format!("canonicalizing PATH binary {}", path_binary.display()))?;
            println!("PATH resolves {TUI_BINARY} to {}", path_binary.display());
            if path_binary != installed {
                bail!(
                    "PATH resolves {TUI_BINARY} to {}, not the freshly installed binary at {}",
                    path_binary.display(),
                    installed.display()
                );
            }
        }
        None => {
            bail!(
                "{TUI_BINARY} was installed at {}, but no {TUI_BINARY} executable was found on PATH",
                installed.display()
            );
        }
    }

    Ok(())
}

fn tui_release(
    workspace: &Path,
    version: Option<String>,
    target: Option<String>,
    allow_dirty: bool,
    skip_sign: bool,
    github_repo: Option<String>,
) -> Result<()> {
    let package_version = package_version(workspace, TUI_PACKAGE)?;
    let version = version.unwrap_or(package_version.clone());
    if version != package_version {
        bail!(
            "release version {version} does not match Cargo.toml package version {package_version}"
        );
    }
    if !allow_dirty {
        ensure_clean_worktree(workspace)?;
    }

    let target = target.unwrap_or(host_target(workspace)?);
    run(
        workspace,
        "cargo",
        [
            "build",
            "-p",
            TUI_PACKAGE,
            "--release",
            "--locked",
            "--target",
            target.as_str(),
        ],
    )?;

    let binary = workspace
        .join("target")
        .join(&target)
        .join("release")
        .join(exe_name(TUI_BINARY));
    if !binary.exists() {
        bail!("expected release binary at {}", binary.display());
    }

    let dist_dir = workspace.join(DIST_DIR);
    fs::create_dir_all(&dist_dir)
        .with_context(|| format!("creating dist directory {}", dist_dir.display()))?;

    let asset_stem = release_asset_stem(&version, &target);
    let archive_path = dist_dir.join(format!("{asset_stem}.tar.gz"));
    create_release_archive(workspace, &binary, &asset_stem, &archive_path)?;
    let checksum_path = write_sha256_sidecar(&archive_path)?;

    if !skip_sign {
        sign_release_file(workspace, &archive_path)?;
        sign_release_file(workspace, &checksum_path)?;
    }

    if let Some(github_repo) = github_repo {
        let formula_path = dist_dir.join("nextdeck.rb");
        write_homebrew_formula(&version, &github_repo, &dist_dir, &formula_path)?;
        println!("Wrote Homebrew formula {}", formula_path.display());
    }

    println!("Release artifact {}", archive_path.display());
    println!("Checksum {}", checksum_path.display());
    Ok(())
}

fn tui_homebrew_formula(
    workspace: &Path,
    version: Option<String>,
    github_repo: &str,
    dist_dir: &Path,
    output: &Path,
) -> Result<()> {
    let version = version.unwrap_or(package_version(workspace, TUI_PACKAGE)?);
    write_homebrew_formula(&version, github_repo, dist_dir, output)?;
    println!("Wrote Homebrew formula {}", output.display());
    Ok(())
}

fn release_asset_stem(version: &str, target: &str) -> String {
    format!("{TUI_BINARY}-v{version}-{target}")
}

fn create_release_archive(
    workspace: &Path,
    binary: &Path,
    asset_stem: &str,
    archive_path: &Path,
) -> Result<()> {
    if archive_path.exists() {
        fs::remove_file(archive_path)
            .with_context(|| format!("removing old archive {}", archive_path.display()))?;
    }

    let archive = File::create(archive_path)
        .with_context(|| format!("creating archive {}", archive_path.display()))?;
    let encoder = GzEncoder::new(archive, Compression::default());
    let mut tar = TarBuilder::new(encoder);

    tar.append_path_with_name(binary, format!("{asset_stem}/{}", exe_name(TUI_BINARY)))
        .with_context(|| format!("adding binary {}", binary.display()))?;

    let readme = workspace.join("README.md");
    if readme.exists() {
        tar.append_path_with_name(&readme, format!("{asset_stem}/README.md"))
            .with_context(|| format!("adding {}", readme.display()))?;
    }

    let license = workspace.join("LICENSE");
    tar.append_path_with_name(&license, format!("{asset_stem}/LICENSE"))
        .with_context(|| format!("adding {}", license.display()))?;

    tar.finish()
        .with_context(|| format!("finishing archive {}", archive_path.display()))?;
    let encoder = tar
        .into_inner()
        .with_context(|| format!("finishing gzip encoder for {}", archive_path.display()))?;
    encoder
        .finish()
        .with_context(|| format!("writing archive {}", archive_path.display()))?;
    Ok(())
}

fn write_sha256_sidecar(path: &Path) -> Result<PathBuf> {
    let digest = sha256_file(path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("archive path has no UTF-8 file name: {}", path.display()))?;
    let sidecar = path.with_file_name(format!("{file_name}.sha256"));
    fs::write(&sidecar, format!("{digest}  {file_name}\n"))
        .with_context(|| format!("writing checksum {}", sidecar.display()))?;
    Ok(sidecar)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push(HEX[(byte >> 4) as usize] as char);
        rendered.push(HEX[(byte & 0x0f) as usize] as char);
    }
    rendered
}

fn sign_release_file(workspace: &Path, path: &Path) -> Result<()> {
    let bundle = format!("{}.sigstore.json", path.display());
    run(
        workspace,
        "cosign",
        [
            "sign-blob",
            "--yes",
            "--bundle",
            bundle.as_str(),
            path.to_str()
                .with_context(|| format!("path is not UTF-8: {}", path.display()))?,
        ],
    )
    .with_context(|| {
        format!(
            "signing {} with cosign; install cosign or pass --skip-sign for local unsigned dry runs",
            path.display()
        )
    })
}

#[derive(Clone, Copy)]
struct FormulaTarget {
    rust_target: &'static str,
    os: FormulaOs,
    cpu: &'static str,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FormulaOs {
    Macos,
    Linux,
}

const FORMULA_TARGETS: &[FormulaTarget] = &[
    FormulaTarget {
        rust_target: "aarch64-apple-darwin",
        os: FormulaOs::Macos,
        cpu: "arm",
    },
    FormulaTarget {
        rust_target: "x86_64-apple-darwin",
        os: FormulaOs::Macos,
        cpu: "intel",
    },
    FormulaTarget {
        rust_target: "aarch64-unknown-linux-gnu",
        os: FormulaOs::Linux,
        cpu: "arm",
    },
    FormulaTarget {
        rust_target: "x86_64-unknown-linux-gnu",
        os: FormulaOs::Linux,
        cpu: "intel",
    },
];

fn write_homebrew_formula(
    version: &str,
    github_repo: &str,
    dist_dir: &Path,
    output: &Path,
) -> Result<()> {
    let mut formula = String::new();
    formula.push_str("class Nextdeck < Formula\n");
    formula.push_str("  desc \"TUI dashboard for cargo-nextest\"\n");
    formula.push_str(&format!(
        "  homepage \"https://github.com/{github_repo}\"\n"
    ));
    formula.push_str(&format!("  version \"{version}\"\n"));
    formula.push_str("  license \"Apache-2.0\"\n\n");
    formula.push_str("  depends_on \"cargo-nextest\"\n\n");

    let mut wrote_any = false;
    wrote_any |= write_formula_os_block(
        &mut formula,
        FormulaOs::Macos,
        version,
        github_repo,
        dist_dir,
    )?;
    wrote_any |= write_formula_os_block(
        &mut formula,
        FormulaOs::Linux,
        version,
        github_repo,
        dist_dir,
    )?;
    if !wrote_any {
        bail!(
            "no release checksums found in {}; run `cargo xtask tui-release` first or download release artifacts",
            dist_dir.display()
        );
    }

    formula.push_str("  def install\n");
    formula.push_str("    bin.install \"nextdeck\"\n");
    formula.push_str("  end\n\n");
    formula.push_str("  test do\n");
    formula
        .push_str("    assert_match version.to_s, shell_output(\"#{bin}/nextdeck --version\")\n");
    formula.push_str("  end\n");
    formula.push_str("end\n");

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating formula directory {}", parent.display()))?;
    }
    fs::write(output, formula).with_context(|| format!("writing formula {}", output.display()))?;
    Ok(())
}

fn write_formula_os_block(
    formula: &mut String,
    os: FormulaOs,
    version: &str,
    github_repo: &str,
    dist_dir: &Path,
) -> Result<bool> {
    let targets = FORMULA_TARGETS
        .iter()
        .copied()
        .filter(|target| target.os == os)
        .filter_map(|target| {
            formula_target_checksum(version, target, dist_dir)
                .transpose()
                .map(|result| result.map(|checksum| (target, checksum)))
        })
        .collect::<Result<Vec<_>>>()?;
    if targets.is_empty() {
        return Ok(false);
    }

    formula.push_str(match os {
        FormulaOs::Macos => "  on_macos do\n",
        FormulaOs::Linux => "  on_linux do\n",
    });

    for (index, (target, checksum)) in targets.iter().enumerate() {
        let branch = if index == 0 { "if" } else { "elsif" };
        formula.push_str(&format!("    {branch} Hardware::CPU.{}?\n", target.cpu));
        let asset = format!("{}.tar.gz", release_asset_stem(version, target.rust_target));
        formula.push_str(&format!(
            "      url \"https://github.com/{github_repo}/releases/download/v{version}/{asset}\"\n"
        ));
        formula.push_str(&format!("      sha256 \"{checksum}\"\n"));
    }
    formula.push_str("    end\n");
    formula.push_str("  end\n\n");
    Ok(true)
}

fn formula_target_checksum(
    version: &str,
    target: FormulaTarget,
    dist_dir: &Path,
) -> Result<Option<String>> {
    let asset = format!("{}.tar.gz", release_asset_stem(version, target.rust_target));
    let checksum_path = dist_dir.join(format!("{asset}.sha256"));
    if !checksum_path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&checksum_path)
        .with_context(|| format!("reading checksum {}", checksum_path.display()))?;
    let mut parts = text.split_whitespace();
    let checksum = parts
        .next()
        .with_context(|| format!("checksum file is empty: {}", checksum_path.display()))?;
    let file_name = parts.next().with_context(|| {
        format!(
            "checksum file is missing file name: {}",
            checksum_path.display()
        )
    })?;
    if file_name != asset {
        bail!(
            "checksum {} references {file_name}, expected {asset}",
            checksum_path.display()
        );
    }
    Ok(Some(checksum.to_owned()))
}

fn host_target(workspace: &Path) -> Result<String> {
    let output = command_stdout_with_cwd(workspace, "rustc", ["-vV"])?;
    output
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(ToOwned::to_owned))
        .context("could not determine rustc host target")
}

fn ensure_clean_worktree(workspace: &Path) -> Result<()> {
    let output = command_stdout_with_cwd(workspace, "git", ["status", "--porcelain"])?;
    if !output.trim().is_empty() {
        bail!("worktree is dirty; commit changes or pass --allow-dirty")
    }
    Ok(())
}

fn cargo_install_bin_path() -> Result<PathBuf> {
    let root = if let Some(root) = env::var_os("CARGO_INSTALL_ROOT") {
        PathBuf::from(root)
    } else if let Some(home) = env::var_os("CARGO_HOME") {
        PathBuf::from(home)
    } else {
        let home = env::var_os("HOME").context("HOME is not set and CARGO_HOME is not set")?;
        PathBuf::from(home).join(".cargo")
    };
    Ok(root.join("bin").join(exe_name(TUI_BINARY)))
}

fn find_on_path(binary: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(exe_name(binary)))
        .find(|candidate| candidate.is_file())
}

fn exe_name(binary: &str) -> String {
    format!("{binary}{}", env::consts::EXE_SUFFIX)
}

fn nextest_args_for_tree_path(workspace: &Path, tree_path: &str) -> Result<Vec<String>> {
    if tree_path == "workspace" {
        return Ok(Vec::new());
    }
    let packages = workspace_package_names(workspace)?;
    let mut parts = tree_path.split("::");
    let Some(first) = parts.next() else {
        return Ok(Vec::new());
    };
    let rest = parts.collect::<Vec<_>>();
    if packages.contains(first) {
        let mut args = vec!["-p".to_owned(), first.to_owned()];
        if !rest.is_empty() {
            args.push(rest.join("::"));
        }
        Ok(args)
    } else {
        Ok(vec![tree_path.to_owned()])
    }
}

fn workspace_package_names(workspace: &Path) -> Result<BTreeSet<String>> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(workspace)
        .output()
        .context("running cargo metadata")?;
    if !output.status.success() {
        bail!("cargo metadata exited with {}", output.status);
    }
    let value =
        serde_json::from_slice::<Value>(&output.stdout).context("parsing cargo metadata")?;
    let packages = value
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|package| package.get("name").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect();
    Ok(packages)
}

fn print_stream_as_jsonl(stream: &str, text: &str) -> Result<()> {
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(json) = trimmed.strip_prefix(nextdeck_helper::FRAME_PREFIX) {
            print_jsonl(match serde_json::from_str::<Value>(json) {
                Ok(event) => json!({ "kind": "event", "stream": stream, "event": event }),
                Err(error) => json!({
                    "kind": "event",
                    "stream": stream,
                    "text": line,
                    "parse_error": error.to_string(),
                }),
            })?;
        } else {
            print_jsonl(json!({ "kind": "stream", "stream": stream, "text": line }))?;
        }
    }
    Ok(())
}

fn print_jsonl(record: Value) -> Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, &record).context("writing JSONL record")?;
    writeln!(stdout).context("writing JSONL newline")
}

fn command_stdout<I, S>(program: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    print_command(&program.display().to_string(), &args);
    let output = Command::new(program)
        .args(&args)
        .output()
        .with_context(|| format!("running {}", program.display()))?;

    if !output.status.success() {
        bail!("{} exited with {}", program.display(), output.status);
    }

    String::from_utf8(output.stdout)
        .with_context(|| format!("{} stdout was not UTF-8", program.display()))
}

fn command_stdout_with_cwd<I, S>(cwd: &Path, program: &str, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    print_command(program, &args);
    let output = Command::new(program)
        .args(&args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("running {program}"))?;

    if !output.status.success() {
        bail!("{program} exited with {}", output.status);
    }

    String::from_utf8(output.stdout).with_context(|| format!("{program} stdout was not UTF-8"))
}

fn package_version(workspace: &Path, package: &str) -> Result<String> {
    let manifest = package_manifest(workspace, package)?;
    let cargo_toml = fs::read_to_string(&manifest)
        .with_context(|| format!("reading {} for package version", manifest.display()))?;
    let mut in_package = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        match trimmed {
            "[package]" => in_package = true,
            "[workspace]" | "[dependencies]" => in_package = false,
            _ => {}
        }

        if in_package && let Some(version) = trimmed.strip_prefix("version = ") {
            return Ok(version.trim_matches('"').to_owned());
        }
    }

    bail!("could not find [package] version in {}", manifest.display())
}

fn package_manifest(workspace: &Path, package: &str) -> Result<PathBuf> {
    match package {
        TUI_PACKAGE => Ok(workspace.join("Cargo.toml")),
        HELPER_PACKAGE => Ok(workspace.join(HELPER_PACKAGE).join("Cargo.toml")),
        _ => bail!("unknown package {package}"),
    }
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
