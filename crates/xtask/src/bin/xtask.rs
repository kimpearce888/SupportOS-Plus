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
    Dev,
    Lint,
    VerifyIcons,
    Package {
        #[arg(long, default_value = "target/appimage")]
        out: String,
        #[arg(long, env = "APPIMAGETOOL")]
        appimagetool: Option<String>,
        #[arg(long)]
        skip_build: bool,
        #[arg(last = true)]
        args: Vec<String>,
    },
    Deb {
        #[arg(long, default_value = "target/deb")]
        out: String,
        #[arg(long)]
        skip_build: bool,
        #[arg(last = true)]
        args: Vec<String>,
    },
    Migrate {
        #[arg(long, default_value_t = default_db_path_str())]
        db: String,
    },
    Backup {
        #[arg(long, default_value_t = default_db_path_str())]
        db: String,
        #[arg(long, default_value = "./data/backups")]
        backups_dir: String,
    },
    Restore {
        #[arg(long, default_value_t = default_db_path_str())]
        db: String,
        #[arg(long)]
        backup: String,
    },
    Healthcheck {
        #[arg(long, default_value_t = default_db_path_str())]
        db: String,
    },
    VectorCount {
        #[arg(long, default_value_t = default_data_dir_str())]
        data_dir: String,
        #[arg(long)]
        db: Option<String>,
    },
    StripComments {
        #[arg(long)]
        check: bool,
        paths: Vec<String>,
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
        Cmd::Healthcheck { db } => run_healthcheck(&db),
        Cmd::VectorCount { data_dir, db } => run_vector_count(&data_dir, db.as_deref()),
        Cmd::StripComments { check, paths } => run_strip_comments(check, &paths),
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

fn default_data_dir_str() -> String {
    spp_core::config::default_data_dir()
        .to_string_lossy()
        .into_owned()
}

fn default_db_path_str() -> String {
    std::path::Path::new(&default_data_dir_str())
        .join(spp_core::config::DB_FILE_NAME)
        .display()
        .to_string()
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
        // AU2-13a: the per-entity count API on the vector store was deleted
        // as program-dead; the diagnostic reads the store's payload table
        // directly (a plain table — no sqlite-vec extension needed).
        let store_db = store_dir.join("vectors.db");
        match rusqlite::Connection::open_with_flags(
            &store_db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(conn) => {
                let mut stmt = conn.prepare(
                    "SELECT entity_type, COUNT(*) FROM vec_payload
                     WHERE collection = 'supportos_vectors'
                     GROUP BY entity_type ORDER BY entity_type",
                )?;
                let rows = stmt
                    .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
                    .collect::<rusqlite::Result<Vec<(String, i64)>>>()?;
                if rows.is_empty() {
                    println!("vector-count:   (no payload rows)");
                }
                for (entity_type, count) in rows {
                    println!("vector-count:   {entity_type:<20}{count}");
                }
            }
            Err(e) => println!("vector-count:   (store unreadable: {e})"),
        }
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

fn run_strip_comments(check: bool, paths: &[String]) -> anyhow::Result<()> {
    let stats = spp_xtask::strip::run(spp_xtask::strip::StripOptions {
        workspace_root: workspace_root(),
        roots: paths.to_vec(),
        check,
    })?;
    if check {
        println!(
            "strip-comments: clean ({}/{} scanned)",
            stats.files_scanned, stats.files_scanned
        );
    } else {
        println!(
            "strip-comments: {} file(s) rewritten, {} whole comment line(s) and {} inline \
             comment(s) removed ({}/{} scanned had comments)",
            stats.files_rewritten,
            stats.whole_lines_removed,
            stats.inline_comments_removed,
            stats.files_with_comments,
            stats.files_scanned
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
