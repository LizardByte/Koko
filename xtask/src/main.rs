use std::{
    collections::{
        HashSet,
        hash_map::DefaultHasher,
    },
    env,
    error::Error,
    fs,
    hash::{
        Hash,
        Hasher,
    },
    io::{
        self,
        Write,
    },
    path::{
        Path,
        PathBuf,
    },
    process::Command,
    time::{
        SystemTime,
        UNIX_EPOCH,
    },
};

const MIGRATIONS_DIR: &str = "crates/server/sql/migrations";
const DB_MODULE: &str = "crates/server/src/db/mod.rs";
const ORDER_MARKER: &str = "const SQLITE_MIGRATION_ORDER: &[&str] = &[";

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    match command {
        "new-migration" => {
            args.remove(0);
            new_migration(args)
        }
        "msi-version" => {
            args.remove(0);
            msi_version(args)
        }
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown xtask command `{other}`").into()),
    }
}

fn new_migration(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    let mut name = None;
    let mut requested_version = None;
    let mut migrations_dir_arg = None;
    let mut db_module_arg = None;
    let mut no_fmt = false;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_new_migration_usage();
                return Ok(());
            }
            "--no-fmt" => no_fmt = true,
            "--version" => {
                requested_version = Some(
                    args.next()
                        .ok_or("missing value after --version")?
                        .trim()
                        .to_owned(),
                );
            }
            "--migrations-dir" => {
                migrations_dir_arg = Some(
                    args.next()
                        .ok_or("missing value after --migrations-dir")?
                        .trim()
                        .to_owned(),
                );
            }
            "--db-module" => {
                db_module_arg = Some(
                    args.next()
                        .ok_or("missing value after --db-module")?
                        .trim()
                        .to_owned(),
                );
            }
            _ if arg.starts_with("--version=") => {
                requested_version = Some(arg["--version=".len()..].trim().to_owned());
            }
            _ if arg.starts_with("--migrations-dir=") => {
                migrations_dir_arg = Some(arg["--migrations-dir=".len()..].trim().to_owned());
            }
            _ if arg.starts_with("--db-module=") => {
                db_module_arg = Some(arg["--db-module=".len()..].trim().to_owned());
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option `{arg}`").into()),
            _ if name.is_none() => name = Some(arg),
            _ => return Err(format!("unexpected extra argument `{arg}`").into()),
        }
    }

    let name = match name {
        Some(name) => name,
        None => prompt("Migration name")?,
    };
    let slug = migration_slug(&name)?;

    let repo_root = repo_root()?;
    let migrations_dir = repo_path(
        &repo_root,
        migrations_dir_arg.as_deref().unwrap_or(MIGRATIONS_DIR),
    );
    let db_module = repo_path(&repo_root, db_module_arg.as_deref().unwrap_or(DB_MODULE));

    let mut existing_versions = existing_migration_versions(&migrations_dir)?;
    existing_versions.extend(ordered_migration_versions(&db_module)?);

    let version = match requested_version {
        Some(version) => validate_requested_version(version, &existing_versions)?,
        None => generate_version(&existing_versions)?,
    };

    let migration_name = format!("{version}_{slug}");
    let migration_dir = migrations_dir.join(&migration_name);
    if migration_dir.exists() {
        return Err(format!(
            "migration directory already exists: {}",
            migration_dir.display()
        )
        .into());
    }

    fs::create_dir_all(&migration_dir)?;
    fs::write(migration_dir.join("up.sql"), "-- Add migration SQL here.\n")?;
    fs::write(
        migration_dir.join("down.sql"),
        "-- Revert migration SQL here.\n",
    )?;
    update_migration_order(&db_module, &version)?;

    if !no_fmt {
        run_rustfmt(&repo_root)?;
    }

    println!("Migration: {migration_name}");
    println!("Path: {}", migration_dir.display());
    println!("Revision: {version}");

    Ok(())
}

fn msi_version(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    let mut version = None;

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_msi_version_usage();
                return Ok(());
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option `{arg}`").into()),
            _ if version.is_none() => version = Some(arg),
            _ => return Err(format!("unexpected extra argument `{arg}`").into()),
        }
    }

    let version = version.ok_or("missing version")?;
    println!("{}", msi_package_version(&version)?);

    Ok(())
}

fn msi_package_version(version: &str) -> Result<String, Box<dyn Error>> {
    let version = version.trim().strip_prefix('v').unwrap_or(version.trim());
    if version.is_empty() {
        return Err("version cannot be empty".into());
    }
    if version.contains('-') || version.contains('+') {
        return Err(
            "MSI package versions must be derived from a release version without prerelease or \
             build metadata"
                .into(),
        );
    }

    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(format!("version `{version}` must have exactly three numeric parts").into());
    }

    let major = parse_version_part(parts[0], "major")?;
    let minor = parse_version_part(parts[1], "minor")?;
    let patch = parse_version_part(parts[2], "patch")?;

    // Match Sunshine's four-field Windows package shape by splitting HHMMSS.
    // WiX warns when CalVer exceeds MSI ProductVersion limits, but cargo-wix
    // fails before WiX can build the installer.
    let build = patch / 100;
    let revision = patch % 100;

    Ok(format!("{major}.{minor}.{build}.{revision}"))
}

fn parse_version_part(
    part: &str,
    label: &str,
) -> Result<u64, Box<dyn Error>> {
    if part.is_empty() {
        return Err(format!("{label} version part cannot be empty").into());
    }
    if part.len() > 1 && part.starts_with('0') {
        return Err(format!("{label} version part `{part}` must not contain leading zeros").into());
    }
    if !part.chars().all(|character| character.is_ascii_digit()) {
        return Err(format!("{label} version part `{part}` must be numeric").into());
    }

    Ok(part.parse()?)
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let current_dir = env::current_dir()?;
    if current_dir.join("Cargo.toml").exists() && current_dir.join("crates/server").exists() {
        return Ok(current_dir);
    }

    let mut dir = current_dir.as_path();
    while let Some(parent) = dir.parent() {
        if parent.join("Cargo.toml").exists() && parent.join("crates/server").exists() {
            return Ok(parent.to_owned());
        }
        dir = parent;
    }

    Err("could not find Koko repository root".into())
}

fn prompt(label: &str) -> Result<String, Box<dyn Error>> {
    print!("{label}: ");
    io::stdout().flush()?;

    let mut value = String::new();
    io::stdin().read_line(&mut value)?;

    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(format!("{label} cannot be empty").into());
    }

    Ok(value)
}

fn repo_path(
    repo_root: &Path,
    path: &str,
) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() { path } else { repo_root.join(path) }
}

fn migration_slug(name: &str) -> Result<String, Box<dyn Error>> {
    let mut slug = String::new();
    let mut last_was_separator = true;

    for character in name.trim().chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_separator = false;
        } else if !last_was_separator {
            slug.push('_');
            last_was_separator = true;
        }
    }

    while slug.ends_with('_') {
        slug.pop();
    }

    if slug.is_empty() {
        return Err("migration name must contain at least one ASCII letter or number".into());
    }

    Ok(slug)
}

fn existing_migration_versions(migrations_dir: &Path) -> Result<HashSet<String>, Box<dyn Error>> {
    let mut versions = HashSet::new();
    if !migrations_dir.exists() {
        return Ok(versions);
    }

    for entry in fs::read_dir(migrations_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some((version, _)) = name.split_once('_') {
            versions.insert(version.to_owned());
        }
    }

    Ok(versions)
}

fn ordered_migration_versions(db_module: &Path) -> Result<HashSet<String>, Box<dyn Error>> {
    Ok(read_migration_order(db_module)?.into_iter().collect())
}

fn read_migration_order(db_module: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let content = fs::read_to_string(db_module)?;
    let start = content
        .find(ORDER_MARKER)
        .ok_or("could not find SQLITE_MIGRATION_ORDER")?;
    let body_start = start + ORDER_MARKER.len();
    let end = content[body_start..]
        .find("];")
        .map(|offset| body_start + offset)
        .ok_or("could not find the end of SQLITE_MIGRATION_ORDER")?;

    Ok(quoted_strings(&content[body_start..end]))
}

fn quoted_strings(input: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = input;

    while let Some(start) = rest.find('"') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('"') else {
            break;
        };
        values.push(after_start[..end].to_owned());
        rest = &after_start[end + 1..];
    }

    values
}

fn validate_requested_version(
    version: String,
    existing_versions: &HashSet<String>,
) -> Result<String, Box<dyn Error>> {
    let version = version.trim().to_ascii_lowercase();
    if version.is_empty() {
        return Err("migration revision cannot be empty".into());
    }
    if !version
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return Err("migration revision must contain only hexadecimal characters".into());
    }
    if existing_versions.contains(&version) {
        return Err(format!("migration revision {version} already exists").into());
    }

    Ok(version)
}

fn generate_version(existing_versions: &HashSet<String>) -> Result<String, Box<dyn Error>> {
    for attempt in 0_u64..1024 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let mut hasher = DefaultHasher::new();
        now.hash(&mut hasher);
        std::process::id().hash(&mut hasher);
        attempt.hash(&mut hasher);

        let version = format!("{:012x}", hasher.finish() & 0x0000_ffff_ffff_ffff);
        if !existing_versions.contains(&version) {
            return Ok(version);
        }
    }

    Err("failed to generate a unique migration revision after 1024 attempts".into())
}

fn update_migration_order(
    db_module: &Path,
    version: &str,
) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(db_module)?;
    let start = content
        .find(ORDER_MARKER)
        .ok_or("could not find SQLITE_MIGRATION_ORDER")?;
    let body_start = start + ORDER_MARKER.len();
    let end = content[body_start..]
        .find("];")
        .map(|offset| body_start + offset)
        .ok_or("could not find the end of SQLITE_MIGRATION_ORDER")?;

    let mut versions = quoted_strings(&content[body_start..end]);
    if versions.iter().any(|existing| existing == version) {
        return Err(format!("migration revision {version} is already listed").into());
    }
    versions.push(version.to_owned());

    let body = versions
        .iter()
        .map(|version| format!("    \"{version}\","))
        .collect::<Vec<_>>()
        .join("\n");
    let replacement = format!("{ORDER_MARKER}\n{body}\n];");

    let updated = format!(
        "{}{}{}",
        &content[..start],
        replacement,
        &content[end + 2..]
    );
    fs::write(db_module, updated)?;

    Ok(())
}

fn run_rustfmt(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let status = Command::new("cargo")
        .arg("+nightly")
        .arg("fmt")
        .current_dir(repo_root)
        .status()?;

    if !status.success() {
        return Err("cargo +nightly fmt failed".into());
    }

    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!("  cargo new-migration [name] [--version <hex>] [--no-fmt]");
    println!("  cargo msi-version <version>");
}

fn print_new_migration_usage() {
    println!("Usage:");
    println!("  cargo new-migration [name] [--version <hex>] [--no-fmt]");
    println!();
    println!("Examples:");
    println!("  cargo new-migration add_media_flags");
    println!("  cargo new-migration add_media_flags --version facefeed1234");
}

fn print_msi_version_usage() {
    println!("Usage:");
    println!("  cargo msi-version <version>");
    println!();
    println!("Examples:");
    println!("  cargo msi-version 2026.624.125525");
}

#[cfg(test)]
mod tests {
    use super::msi_package_version;

    #[test]
    fn msi_package_version_splits_calver_time() {
        assert_eq!(
            msi_package_version("2026.624.125525").unwrap(),
            "2026.624.1255.25"
        );
    }

    #[test]
    fn msi_package_version_allows_v_prefix() {
        assert_eq!(
            msi_package_version("v2026.622.124200").unwrap(),
            "2026.622.1242.0"
        );
    }

    #[test]
    fn msi_package_version_handles_workspace_default() {
        assert_eq!(msi_package_version("0.0.0").unwrap(), "0.0.0.0");
    }

    #[test]
    fn msi_package_version_rejects_prerelease_versions() {
        assert!(msi_package_version("2026.624.125525-beta.1").is_err());
    }

    #[test]
    fn msi_package_version_rejects_leading_zeros() {
        assert!(msi_package_version("2026.0624.125525").is_err());
    }
}
