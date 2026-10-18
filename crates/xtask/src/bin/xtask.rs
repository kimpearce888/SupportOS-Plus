use std::process::Command;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", version, about = "SupportOS++ developer entry point", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Dev,
    TrunkServe,
    TrunkBuild,
    Lint,
    VerifyIcons,
    Package {
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
        /// App data directory hosting the embedded qdrant store.
        #[arg(long, default_value_t = default_data_dir_str())]
        data_dir: String,
        /// Qdrant URL identifying the embedded store directory.
        #[arg(long, default_value_t = default_qdrant_url())]
        url: String,
        /// Optional SQLite database path for a chunk-row cross-check.
        #[arg(long)]
        db: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Dev => run_dev(),
        Cmd::TrunkServe => run_trunk("serve"),
        Cmd::TrunkBuild => run_trunk_release_build(),
        Cmd::Lint => run_lint(),
        Cmd::VerifyIcons => run_verify_icons(),
        Cmd::Package { args } => run_package(&args),
        Cmd::Migrate { db } => run_migrate(&db),
        Cmd::Backup { db, backups_dir } => run_backup(&db, &backups_dir),
        Cmd::Restore { db, backup } => run_restore(&db, &backup),
        Cmd::Seed { db } => run_seed(&db),
        Cmd::Healthcheck { db } => run_healthcheck(&db),
        Cmd::VectorCount { data_dir, url, db } => run_vector_count(&data_dir, &url, db.as_deref()),
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
    let tauri_dir = workspace_root()
        .join("crates")
        .join("app")
        .join("src-tauri");
    let status = Command::new("cargo")
        .arg("tauri")
        .arg("dev")
        .current_dir(tauri_dir)
        .status()?;
    anyhow::ensure!(status.success(), "cargo tauri dev failed");
    Ok(())
}

fn run_trunk(mode: &str) -> anyhow::Result<()> {
    let ui_dir = workspace_root().join("crates").join("ui");
    let status = Command::new("trunk")
        .arg(mode)
        .current_dir(ui_dir)
        .status()?;
    anyhow::ensure!(status.success(), "trunk {mode} failed");
    Ok(())
}

fn run_trunk_release_build() -> anyhow::Result<()> {
    let ui_dir = workspace_root().join("crates").join("ui");
    let status = Command::new("trunk")
        .args(["build", "--release"])
        .current_dir(ui_dir)
        .status()?;
    anyhow::ensure!(status.success(), "trunk build --release failed");
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
    let tauri_dir = workspace_root()
        .join("crates")
        .join("app")
        .join("src-tauri");
    spp_xtask::icons::verify_icons(&tauri_dir)?;
    println!("icon set verified: png/ico/icns structures and the bundle.icon list are consistent");
    Ok(())
}

fn run_package(extra_args: &[String]) -> anyhow::Result<()> {
    let tauri_dir = workspace_root()
        .join("crates")
        .join("app")
        .join("src-tauri");
    let mut cmd = Command::new("cargo");
    cmd.arg("tauri").arg("build");
    for arg in extra_args {
        cmd.arg(arg);
    }
    let status = cmd.current_dir(tauri_dir).status()?;
    anyhow::ensure!(status.success(), "cargo tauri build failed");
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

fn default_qdrant_url() -> String {
    spp_core::config::load_env_config().qdrant.url
}

const D4_GATE_CHUNKS: usize = 100_000;

fn run_vector_count(data_dir: &str, url: &str, db: Option<&str>) -> anyhow::Result<()> {
    let dir = std::path::Path::new(data_dir);
    let store_dir = spp_core::vectorstore_qdrant::embedded_store_dir(dir, url);
    println!("vector-count: data dir      {}", dir.display());
    println!("vector-count: qdrant url     {url}");
    println!("vector-count: embedded store {}", store_dir.display());

    let qdrant = spp_core::vectorstore_qdrant::EmbeddedQdrant::new(dir, url, true);
    let total = if store_dir.exists() {
        match qdrant.try_point_count()? {
            Some(n) => n,
            None => {
                println!("vector-count: collection 'supportos_vectors' not found (0 points)");
                0
            }
        }
    } else {
        println!("vector-count: embedded store directory not found (0 points)");
        0
    };
    println!("vector-count: total points  {total}");

    if total > 0 {
        let knowledge =
            qdrant.count_by_entity(spp_core::embeddings::entity_type_for("knowledge_chunks"));
        let docs = qdrant.count_by_entity(spp_core::embeddings::entity_type_for("docs_chunks"));
        let conversations =
            qdrant.count_by_entity(spp_core::embeddings::entity_type_for("conversation_chunks"));
        println!("vector-count:   knowledge_chunk     {knowledge}");
        println!("vector-count:   docs_chunk          {docs}");
        println!("vector-count:   conversation_chunk  {conversations}");
    }

    let db_path = db.map(String::from).unwrap_or_else(|| {
        dir.join("supportos-plusplus.db")
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
        println!("vector-count: D4 gate -> {total} < {D4_GATE_CHUNKS} chunks: sqlite-vec GO");
    } else {
        println!("vector-count: D4 gate -> {total} >= {D4_GATE_CHUNKS} chunks: keep qdrant-edge");
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
