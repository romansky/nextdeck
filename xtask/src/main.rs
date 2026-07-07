use std::{
    env,
    ffi::OsString,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use tar::Builder as TarBuilder;

const APP_PACKAGE: &str = "nextdeck";
const APP_BINARY: &str = "nextdeck";
const DIST_DIR: &str = "target/dist";

#[derive(Debug, Parser)]
#[command(version, about = "Local project automation")]
struct Cli {
    #[command(subcommand)]
    command: XtaskCommand,
}

#[derive(Debug, Subcommand)]
enum XtaskCommand {
    #[command(about = "Print nextdeck xtask integration metadata")]
    NextdeckInfo {
        #[arg(long, value_enum, default_value_t = XtaskInfoFormat::Json)]
        format: XtaskInfoFormat,
    },
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
    #[command(about = "Build, archive, checksum, and sign a release artifact")]
    Release {
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
    #[command(about = "Generate a Homebrew formula from release artifact checksums")]
    HomebrewFormula {
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum XtaskInfoFormat {
    Json,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace = workspace_root()?;

    match cli.command {
        XtaskCommand::NextdeckInfo { format } => nextdeck_info(format),
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
        XtaskCommand::Release {
            version,
            target,
            allow_dirty,
            skip_sign,
            github_repo,
        } => release(
            &workspace,
            version,
            target,
            allow_dirty,
            skip_sign,
            github_repo,
        ),
        XtaskCommand::HomebrewFormula {
            version,
            github_repo,
            dist_dir,
            output,
        } => homebrew_formula(&workspace, version, &github_repo, &dist_dir, &output),
    }
}

fn nextdeck_info(format: XtaskInfoFormat) -> Result<()> {
    match format {
        XtaskInfoFormat::Json => {
            let manifest = serde_json::json!({
                "schema_version": 1,
                "commands": [
                    {
                        "name": "check",
                        "about": "Run local checks expected before publishing",
                        "args": [
                            bool_arg("allow-dirty", "Allow cargo package to run with a dirty worktree")
                        ]
                    },
                    {
                        "name": "package",
                        "about": "Create a local .crate package in target/package",
                        "args": [
                            bool_arg("allow-dirty", "Allow packaging with a dirty worktree")
                        ]
                    },
                    {
                        "name": "install-path",
                        "about": "Install the app locally from the current workspace",
                        "args": []
                    },
                    {
                        "name": "install-package",
                        "about": "Install the app locally from the generated .crate package",
                        "args": [
                            bool_arg("allow-dirty", "Allow packaging with a dirty worktree first")
                        ]
                    },
                    {
                        "name": "publish-local",
                        "about": "Package and install the verified package locally",
                        "args": [
                            bool_arg("allow-dirty", "Allow packaging with a dirty worktree")
                        ]
                    },
                    {
                        "name": "release",
                        "about": "Build, archive, checksum, and sign a release artifact",
                        "args": [
                            string_arg("version", false, "Release version. Defaults to the root Cargo.toml version"),
                            string_arg("target", false, "Rust target triple. Defaults to the host target"),
                            bool_arg("allow-dirty", "Allow release artifacts with a dirty worktree"),
                            bool_arg("skip-sign", "Create artifacts without cosign signatures"),
                            string_arg("github-repo", false, "GitHub repository as owner/repo, used to generate a Homebrew formula")
                        ]
                    },
                    {
                        "name": "homebrew-formula",
                        "about": "Generate a Homebrew formula from release artifact checksums",
                        "args": [
                            string_arg("version", false, "Formula version. Defaults to the root Cargo.toml version"),
                            string_arg("github-repo", true, "GitHub repository as owner/repo"),
                            string_arg_with_default("dist-dir", "target/dist", "Directory containing *.tar.gz.sha256 files"),
                            string_arg("output", true, "Output formula path")
                        ]
                    }
                ]
            });
            serde_json::to_writer_pretty(std::io::stdout(), &manifest)?;
            println!();
            Ok(())
        }
    }
}

fn bool_arg(name: &str, help: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "long": name,
        "help": help,
        "value": { "type": "bool", "default": false }
    })
}

fn string_arg(name: &str, required: bool, help: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "long": name,
        "required": required,
        "help": help,
        "value": { "type": "string" }
    })
}

fn string_arg_with_default(name: &str, default: &str, help: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "long": name,
        "help": help,
        "value": { "type": "string", "default": default }
    })
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
    let package_verify_target = workspace.join("target/package-verify");
    let package_dir = package_verify_target.join("package");
    let crate_path = package_dir.join(format!("{APP_PACKAGE}-{version}.crate"));
    let unpacked_dir = package_dir.join(format!("{APP_PACKAGE}-{version}"));
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
    )?;
    verify_local_install()
}

fn install_crate(workspace: &Path, crate_path: &Path) -> Result<()> {
    let install_dir = isolated_package_dir(crate_path)?;
    copy_dir(crate_path, &install_dir)?;
    let crate_arg = install_dir
        .to_str()
        .with_context(|| format!("package path is not UTF-8: {}", install_dir.display()))?;
    run(
        workspace,
        "cargo",
        ["install", "--path", crate_arg, "--locked", "--force"],
    )?;
    verify_local_install()
}

fn isolated_package_dir(crate_path: &Path) -> Result<PathBuf> {
    let package_name = crate_path
        .file_name()
        .context("verified package directory has no file name")?;
    let root = env::temp_dir().join(format!("{APP_PACKAGE}-publish-local"));
    let install_dir = root.join(package_name);

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir)
            .with_context(|| format!("removing old isolated package {}", install_dir.display()))?;
    }
    fs::create_dir_all(&root)
        .with_context(|| format!("creating isolated package root {}", root.display()))?;

    Ok(install_dir)
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

    let path_binary = find_on_path(APP_BINARY);
    let installed = fs::canonicalize(&installed)
        .with_context(|| format!("canonicalizing installed binary {}", installed.display()))?;
    let version = command_stdout(&installed, ["--version"])?;

    println!("Installed {} at {}", version.trim(), installed.display());

    match path_binary {
        Some(path_binary) => {
            let path_binary = fs::canonicalize(&path_binary)
                .with_context(|| format!("canonicalizing PATH binary {}", path_binary.display()))?;
            println!("PATH resolves {APP_BINARY} to {}", path_binary.display());
            if path_binary != installed {
                bail!(
                    "PATH resolves {APP_BINARY} to {}, not the freshly installed binary at {}",
                    path_binary.display(),
                    installed.display()
                );
            }
        }
        None => {
            bail!(
                "{APP_BINARY} was installed at {}, but no {APP_BINARY} executable was found on PATH",
                installed.display()
            );
        }
    }

    Ok(())
}

fn release(
    workspace: &Path,
    version: Option<String>,
    target: Option<String>,
    allow_dirty: bool,
    skip_sign: bool,
    github_repo: Option<String>,
) -> Result<()> {
    let package_version = package_version(workspace)?;
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
            APP_PACKAGE,
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
        .join(exe_name(APP_BINARY));
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

fn homebrew_formula(
    workspace: &Path,
    version: Option<String>,
    github_repo: &str,
    dist_dir: &Path,
    output: &Path,
) -> Result<()> {
    let version = version.unwrap_or(package_version(workspace)?);
    write_homebrew_formula(&version, github_repo, dist_dir, output)?;
    println!("Wrote Homebrew formula {}", output.display());
    Ok(())
}

fn release_asset_stem(version: &str, target: &str) -> String {
    format!("{APP_BINARY}-v{version}-{target}")
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

    tar.append_path_with_name(binary, format!("{asset_stem}/{}", exe_name(APP_BINARY)))
        .with_context(|| format!("adding binary {}", binary.display()))?;

    let readme = workspace.join("README.md");
    if readme.exists() {
        tar.append_path_with_name(&readme, format!("{asset_stem}/README.md"))
            .with_context(|| format!("adding {}", readme.display()))?;
    }

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
    formula.push_str("  license \"MIT OR Apache-2.0\"\n\n");
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
            "no release checksums found in {}; run `cargo xtask release` first or download release artifacts",
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
    Ok(root.join("bin").join(exe_name(APP_BINARY)))
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

        if in_package && let Some(version) = trimmed.strip_prefix("version = ") {
            return Ok(version.trim_matches('"').to_owned());
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
