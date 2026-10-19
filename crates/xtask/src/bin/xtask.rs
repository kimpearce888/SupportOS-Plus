use std::process::Command;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", version, about = "SupportOS+ developer entry point", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the native shell (egui) in debug mode.
    Dev,
    Lint,
    VerifyIcons,
    /// Build the release shell and pack it into a type-2 AppImage.
    Package {
        /// Output directory (default: target/appimage).
        #[arg(long, default_value = "target/appimage")]
        out: String,
        /// Path to an existing appimagetool (downloaded when absent).
        #[arg(long, env = "APPIMAGETOOL")]
        appimagetool: Option<String>,
        /// Skip the cargo release build (the binary already exists).
        #[arg(long)]
        skip_build: bool,
        /// Extra arguments forwarded to `cargo build`.
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Build the release shell and pack it into a .deb package.
    Deb {
        /// Output directory (default: target/deb).
        #[arg(long, default_value = "target/deb")]
        out: String,
        /// Skip the cargo release build (the binary already exists).
        #[arg(long)]
        skip_build: bool,
        /// Extra arguments forwarded to `cargo build`.
        #[arg(last = true)]
        args: Vec<String>,
    },
    Migrate {
        #[arg(long, default_value = "./data/supportos.db")]
        db: String,
    },
    Backup {
        #[arg(long, default_value = "./data/supportos.db")]
        db: String,
        #[arg(long, default_value = "./data/backups")]
        backups_dir: String,
    },
    Restore {
        #[arg(long, default_value = "./data/supportos.db")]
        db: String,
        #[arg(long)]
        backup: String,
    },
    Seed {
        #[arg(long, default_value = "./data/supportos.db")]
        db: String,
    },
    Healthcheck {
        #[arg(long, default_value = "./data/supportos.db")]
        db: String,
    },
    /// Count vector-store chunks and evaluate the D4 sqlite-vec gate (< 100k = GO).
    VectorCount {
        /// App data directory hosting the sqlite-vec vector store.
        #[arg(long, default_value_t = default_data_dir_str())]
        data_dir: String,
        /// Optional SQLite database path for a chunk-row cross-check.
        #[arg(long)]
        db: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Dev => run_dev(),
        Cmd::Lint => run_lint(),
        Cmd::VerifyIcons => run_verify_icons(),
        Cmd::Package {
            out,
            appimagetool,
            skip_build,
            args,
        } => run_package(&out, appimagetool.as_deref(), skip_build, &args),
        Cmd::Deb {
            out,
            skip_build,
            args,
        } => run_deb(&out, skip_build, &args),
        Cmd::Migrate { db } => run_migrate(&db),
        Cmd::Backup { db, backups_dir } => run_backup(&db, &backups_dir),
        Cmd::Restore { db, backup } => run_restore(&db, &backup),
        Cmd::Seed { db } => run_seed(&db),
        Cmd::Healthcheck { db } => run_healthcheck(&db),
        Cmd::VectorCount { data_dir, db } => run_vector_count(&data_dir, db.as_deref()),
    }
}

fn workspace_root() -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_string());
    let manifest_dir = std::path::PathBuf::from(manifest);
    manifest_dir
        .ancestors()
        .nth(2)
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap())
}

fn run_dev() -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .args(["run", "-p", "supportos-plus-app-shell"])
        .current_dir(workspace_root())
        .status()?;
    anyhow::ensure!(
        status.success(),
        "cargo run -p supportos-plus-app-shell failed"
    );
    Ok(())
}

fn run_lint() -> anyhow::Result<()> {
    let root = workspace_root();

    let fmt_status = Command::new("cargo")
        .args(["fmt", "--all", "--", "--check"])
        .current_dir(&root)
        .status()?;
    anyhow::ensure!(fmt_status.success(), "rustfmt --check failed");

    let clippy_status = Command::new("cargo")
        .args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
        .current_dir(&root)
        .status()?;
    anyhow::ensure!(clippy_status.success(), "clippy -D warnings failed");

    Ok(())
}

fn run_verify_icons() -> anyhow::Result<()> {
    let shell_dir = workspace_root().join("crates").join("app-shell");
    spp_xtask::icons::verify_icons(&shell_dir)?;
    println!("icon set verified: png/ico/icns structures are consistent");
    Ok(())
}

fn run_package(
    out_dir: &str,
    appimagetool: Option<&str>,
    skip_build: bool,
    extra_args: &[String],
) -> anyhow::Result<()> {
    // Row 7: the Tauri bundler (deb/msi/nsis/dmg/app) is gone; the Linux
    // desktop artifact is a single type-2 AppImage via appimagetool.
    let out = spp_xtask::package::package(spp_xtask::package::PackageOptions {
        workspace_root: workspace_root(),
        out_dir: workspace_root().join(out_dir),
        appimagetool: appimagetool.map(std::path::PathBuf::from),
        extra_cargo_args: extra_args.to_vec(),
        skip_build,
    })?;
    println!("package: AppImage at {}", out.display());
    Ok(())
}

fn run_deb(out_dir: &str, skip_build: bool, extra_args: &[String]) -> anyhow::Result<()> {
    // Debian policy naming: <package>_<version>_<arch>.deb, produced by
    // `dpkg-deb --root-owner-group` from the staged tree.
    let out = spp_xtask::package::package_deb(spp_xtask::package::DebOptions {
        workspace_root: workspace_root(),
        out_dir: workspace_root().join(out_dir),
        extra_cargo_args: extra_args.to_vec(),
        skip_build,
    })?;
    println!("deb: package at {}", out.display());
    Ok(())
}

fn run_migrate(db_path: &str) -> anyhow::Result<()> {
    let path = std::path::Path::new(db_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = spp_core::db::open(path)?;
    spp_core::bootstrap::apply_all(&mut conn)?;
    let version = spp_core::db::latest_version(&conn)?;
    println!(
        "migrate: database at {} is now at schema version {version}",
        path.display()
    );
    Ok(())
}

fn run_backup(db_path: &str, backups_dir: &str) -> anyhow::Result<()> {
    let db = std::path::Path::new(db_path);
    let bdir = std::path::Path::new(backups_dir);
    std::fs::create_dir_all(bdir)?;
    let conn = spp_core::db::open(db)?;
    let result = spp_core::backup_service::backup(&conn, bdir);
    if result.ok {
        if let Some(p) = &result.path {
            println!("backup: created at {p}");
        } else {
            println!("backup: created");
        }
    } else {
        anyhow::bail!("backup failed: {}", result.message);
    }
    Ok(())
}

fn run_restore(db_path: &str, backup_path: &str) -> anyhow::Result<()> {
    let db = std::path::Path::new(db_path);
    let bk = std::path::Path::new(backup_path);
    if !bk.exists() {
        anyhow::bail!("backup file not found: {}", bk.display());
    }
    let result = spp_core::backup_service::restore(db, bk);
    if result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        println!("restore: database restored from {}", bk.display());
    } else {
        anyhow::bail!(
            "restore failed: {}",
            result
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );
    }
    Ok(())
}

fn run_seed(db_path: &str) -> anyhow::Result<()> {
    let path = std::path::Path::new(db_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = spp_core::db::open(path)?;
    spp_core::bootstrap::apply_all(&mut conn)?;
    let seeded = spp_core::demo::seed_demo_data(&conn, true);
    if seeded {
        println!("seed: demo data inserted into {}", path.display());
    } else {
        println!(
            "seed: demo data already present in {} (no changes)",
            path.display()
        );
    }
    Ok(())
}

fn default_data_dir_str() -> String {
    spp_core::config::default_data_dir()
        .to_string_lossy()
        .into_owned()
}

const D4_GATE_CHUNKS: usize = 100_000;

fn run_vector_count(data_dir: &str, db: Option<&str>) -> anyhow::Result<()> {
    let dir = std::path::Path::new(data_dir);
    let store_dir = spp_core::vectorstore_sqlitevec::store_dir(dir);
    println!("vector-count: data dir      {}", dir.display());
    println!(
        "vector-count: sqlite-vec    {}",
        store_dir.join("vectors.db").display()
    );

    let index = spp_core::vectorstore_sqlitevec::VectorIndex::new(dir, true);
    let total = if store_dir.exists() {
        match index.try_point_count()? {
            Some(n) => n,
            None => {
                println!("vector-count: collection 'supportos_vectors' not found (0 points)");
                0
            }
        }
    } else {
        println!("vector-count: sqlite-vec store directory not found (0 points)");
        0
    };
    println!("vector-count: total points  {total}");

    if total > 0 {
        let knowledge =
            index.count_by_entity(spp_core::embeddings::entity_type_for("knowledge_chunks"));
        let docs = index.count_by_entity(spp_core::embeddings::entity_type_for("docs_chunks"));
        let conversations =
            index.count_by_entity(spp_core::embeddings::entity_type_for("conversation_chunks"));
        println!("vector-count:   knowledge_chunk     {knowledge}");
        println!("vector-count:   docs_chunk          {docs}");
        println!("vector-count:   conversation_chunk  {conversations}");
    }

    let db_path = db.map(String::from).unwrap_or_else(|| {
        dir.join(spp_core::config::DB_FILE_NAME)
            .to_string_lossy()
            .into_owned()
    });
    let db_file = std::path::Path::new(&db_path);
    if db_file.exists() {
        let conn = spp_core::db::open(db_file)?;
        let rows = |table: &str| -> i64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(0)
        };
        let indexed = |table: &str| -> i64 {
            conn.query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE embedding_state = 'indexed'"),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0)
        };
        println!("vector-count: sqlite cross-check {}", db_file.display());
        println!(
            "vector-count:   conversation_chunks rows {} (indexed {})",
            rows("conversation_chunks"),
            indexed("conversation_chunks")
        );
        println!(
            "vector-count:   docs_chunks rows {} (indexed {})",
            rows("docs_chunks"),
            indexed("docs_chunks")
        );
        println!(
            "vector-count:   knowledge_chunks rows {} (indexed {})",
            rows("knowledge_chunks"),
            indexed("knowledge_chunks")
        );
    } else {
        println!("vector-count: sqlite cross-check skipped ({db_path} not found)");
    }

    if total < D4_GATE_CHUNKS {
        println!(
            "vector-count: D4 gate -> {total} < {D4_GATE_CHUNKS} chunks: sqlite-vec confirmed (swap landed)"
        );
    } else {
        println!(
            "vector-count: D4 gate -> {total} >= {D4_GATE_CHUNKS} chunks: above gate — revisit D4 \
             (the VectorStore seam stays swappable)"
        );
    }
    Ok(())
}

fn run_healthcheck(db_path: &str) -> anyhow::Result<()> {
    let path = std::path::Path::new(db_path);
    if !path.exists() {
        anyhow::bail!("database file not found: {}", path.display());
    }
    let conn = spp_core::db::open(path)?;
    let version = spp_core::db::latest_version(&conn)?;
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let error_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM application_errors", [], |r| r.get(0))
        .unwrap_or(0);
    println!("healthcheck: OK");
    println!("  database:   {}", path.display());
    println!("  schema:     version {version}");
    println!("  tables:     {table_count}");
    println!("  errors:     {error_count} (in application_errors)");
    Ok(())
}
