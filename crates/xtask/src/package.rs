//! AppImage packaging — `xtask package` (migration-map row 7).
//!
//! The Tauri bundler and its deb/msi/nsis/dmg/app targets left with the
//! Phase 4 cutover; Linux desktop distribution is a single type-2 AppImage.
//! Packaging is two testable halves:
//!
//! * [`stage_appdir`] — builds the AppDir layout (binary, `.desktop`
//!   entry, hicolor icons, `AppRun`) on disk; unit-tested directly.
//! * [`package`] — release build + staging + running `appimagetool`
//!   (located via `APPIMAGETOOL`, cached under `target/tools/`, or
//!   downloaded once from the official continuous release; run with
//!   `APPIMAGE_EXTRACT_AND_RUN=1` so it works on FUSE-less builders).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Human-facing application name (window title / desktop entry).
pub const APP_NAME: &str = "SupportOs-Plus";

/// The app-shell binary name (crates/app-shell `[[bin]]`).
pub const BINARY_NAME: &str = "supportos-plusplus";

/// Icon sizes shipped into the AppDir's hicolor tree.
const ICON_SIZES: &[(&str, &str)] = &[
    ("32x32", "32x32.png"),
    ("128x128", "128x128.png"),
    ("256x256", "256x256.png"),
    ("1024x1024", "1024x1024.png"),
];

const APPIMAGETOOL_URL: &str = "https://github.com/AppImage/appimagetool/releases/download/\
                                continuous/appimagetool-x86_64.AppImage";

/// Everything `package` needs.
pub struct PackageOptions {
    /// Workspace root (the directory of the top-level `Cargo.toml`).
    pub workspace_root: PathBuf,
    /// Where the resulting AppImage lands.
    pub out_dir: PathBuf,
    /// Explicit appimagetool override (CI caches it); downloaded when
    /// absent.
    pub appimagetool: Option<PathBuf>,
    /// Extra arguments forwarded to the `cargo build` invocation.
    pub extra_cargo_args: Vec<String>,
    /// Skips the cargo build (already built, e.g. CI re-runs).
    pub skip_build: bool,
}

/// Build, stage, and pack the AppImage. Returns the artifact path.
pub fn package(opts: PackageOptions) -> anyhow::Result<PathBuf> {
    if !opts.skip_build {
        let mut cmd = Command::new("cargo");
        cmd.args(["build", "--release", "-p", "supportos-plusplus-app-shell"])
            .current_dir(&opts.workspace_root);
        for arg in &opts.extra_cargo_args {
            cmd.arg(arg);
        }
        let status = cmd.status()?;
        anyhow::ensure!(status.success(), "release build failed");
    }

    let appdir = opts.out_dir.join(format!("{BINARY_NAME}.AppDir"));
    stage_appdir(&opts.workspace_root, &appdir)?;

    let tool = resolve_appimagetool(&opts.workspace_root, opts.appimagetool.as_deref())?;
    let output = opts.out_dir.join(format!("{APP_NAME}-x86_64.AppImage"));
    if output.exists() {
        fs::remove_file(&output)?;
    }

    // ARCH pins the runtime; EXTRACT_AND_RUN lets the tool itself run on
    // FUSE-less builders; NO_STRIP keeps the appimage working under
    // `strip = true` release profiles.
    let status = Command::new(&tool)
        .arg(&appdir)
        .arg(&output)
        .env("ARCH", "x86_64")
        .env("APPIMAGE_EXTRACT_AND_RUN", "1")
        .env("NO_STRIP", "1")
        .status()?;
    anyhow::ensure!(
        status.success() && output.exists(),
        "appimagetool failed (status {status})"
    );
    Ok(output)
}

/// Stage the AppDir: binary, desktop entry, icons, AppRun.
///
/// Layout:
///
/// ```text
/// supportos-plusplus.AppDir/
/// ├── AppRun                          (sh shim, execs usr/bin/…)
/// ├── supportos-plusplus.png          (root icon)
/// ├── .DirIcon                        -> root icon
/// ├── usr/bin/supportos-plusplus
/// ├── usr/share/applications/supportos-plusplus.desktop
/// └── usr/share/icons/hicolor/<size>/apps/supportos-plusplus.png
/// ```
pub fn stage_appdir(workspace_root: &Path, appdir: &Path) -> anyhow::Result<()> {
    let bin_src = workspace_root
        .join("target")
        .join("release")
        .join(BINARY_NAME);
    anyhow::ensure!(
        bin_src.is_file(),
        "release binary not found at {} — build first",
        bin_src.display()
    );

    let icons_dir = workspace_root
        .join("crates")
        .join("app-shell")
        .join("icons");

    let bin_dst = appdir.join("usr").join("bin").join(BINARY_NAME);
    let apps_dir = appdir.join("usr").join("share").join("applications");
    let hicolor = appdir
        .join("usr")
        .join("share")
        .join("icons")
        .join("hicolor");
    for dir in [bin_dst.parent().unwrap(), &apps_dir, appdir] {
        fs::create_dir_all(dir)?;
    }

    // Copy the binary as executable.
    fs::copy(&bin_src, &bin_dst)?;
    make_executable(&bin_dst)?;

    // Desktop entry (Exec points at the AppDir-relative path; AppRun is
    // the entry the file manager launches).
    let desktop = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={APP_NAME}\n\
         Comment=Offline-first Help Scout support desk\n\
         Exec=usr/bin/{BINARY_NAME}\n\
         Icon={BINARY_NAME}\n\
         Terminal=false\n\
         Categories=Office;Utility;\n\
         StartupWMClass={APP_NAME}\n"
    );
    fs::write(apps_dir.join(format!("{BINARY_NAME}.desktop")), &desktop)?;
    // appimagetool looks the desktop entry up at the AppDir root; keep the
    // hicolor copy too so the layout follows both conventions.
    fs::write(appdir.join(format!("{BINARY_NAME}.desktop")), desktop)?;

    // hicolor icons from the checked-in set.
    for (size, file) in ICON_SIZES {
        let src = icons_dir.join(file);
        anyhow::ensure!(
            src.is_file(),
            "missing icon {} — run `cargo xtask verify-icons`",
            src.display()
        );
        let dst_dir = hicolor.join(size).join("apps");
        fs::create_dir_all(&dst_dir)?;
        fs::copy(&src, dst_dir.join(format!("{BINARY_NAME}.png")))?;
    }

    // Root icon + .DirIcon (what file managers show).
    let root_icon = appdir.join(format!("{BINARY_NAME}.png"));
    fs::copy(icons_dir.join("256x256.png"), &root_icon)?;
    let dir_icon = appdir.join(".DirIcon");
    copy_or_link(&root_icon, &dir_icon)?;

    // AppRun shim: resolve the real AppDir even when symlinked from
    // $HOME/.local/bin after extraction.
    let apprun = format!(
        "#!/bin/sh\n\
         HERE=\"$(dirname \"$(readlink -f \"$0\")\")\"\n\
         exec \"$HERE/usr/bin/{BINARY_NAME}\" \"$@\"\n"
    );
    let apprun_path = appdir.join("AppRun");
    fs::write(&apprun_path, apprun)?;
    make_executable(&apprun_path)?;

    Ok(())
}

fn copy_or_link(src: &Path, dst: &Path) -> anyhow::Result<()> {
    // A plain copy keeps the AppDir self-contained when it is moved
    // around; hard-link/symlink would break for `cp -r` consumers.
    fs::copy(src, dst)?;
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> anyhow::Result<()> {
    anyhow::bail!("AppImage packaging is Linux-only")
}

/// Locate appimagetool: explicit override → cached download → fresh
/// download into the cache.
fn resolve_appimagetool(
    workspace_root: &Path,
    override_path: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_path {
        anyhow::ensure!(path.is_file(), "APPIMAGETOOL not found: {}", path.display());
        return Ok(path.to_path_buf());
    }
    let cached = workspace_root
        .join("target")
        .join("tools")
        .join("appimagetool-x86_64.AppImage");
    if cached.is_file() {
        return Ok(cached);
    }
    download_appimagetool(&cached)?;
    Ok(cached)
}

fn download_appimagetool(dest: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    eprintln!("package: downloading appimagetool from the continuous release…");
    let bytes = reqwest::blocking::get(APPIMAGETOOL_URL)?
        .error_for_status()?
        .bytes()?;
    fs::write(dest, &bytes)?;
    make_executable(dest)?;
    eprintln!("package: appimagetool cached at {}", dest.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-package-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp root");
        dir
    }

    fn fake_release_build(root: &Path) {
        let target = root.join("target").join("release");
        fs::create_dir_all(&target).expect("create target dir");
        fs::write(target.join(BINARY_NAME), b"#!/bin/sh\nexit 0\n").expect("fake binary");
    }

    fn real_icons(root: &Path) {
        let icons = root.join("crates").join("app-shell").join("icons");
        fs::create_dir_all(&icons).expect("create icons dir");
        for (_, file) in ICON_SIZES {
            fs::write(icons.join(file), b"\x89PNG fake").expect("fake icon");
        }
    }

    #[test]
    fn stage_appdir_builds_the_full_layout() {
        let root = temp_root("layout");
        fake_release_build(&root);
        real_icons(&root);

        let appdir = root.join("out").join(format!("{BINARY_NAME}.AppDir"));
        stage_appdir(&root, &appdir).expect("stage");

        assert!(appdir.join("AppRun").is_file());
        assert!(appdir.join(".DirIcon").is_file());
        assert!(appdir.join(format!("{BINARY_NAME}.png")).is_file());
        assert!(appdir.join("usr").join("bin").join(BINARY_NAME).is_file());
        let desktop = appdir
            .join("usr")
            .join("share")
            .join("applications")
            .join(format!("{BINARY_NAME}.desktop"));
        let desktop = fs::read_to_string(desktop).expect("desktop entry");
        assert!(desktop.contains(&format!("Name={APP_NAME}")));
        assert!(desktop.contains(&format!("Exec=usr/bin/{BINARY_NAME}")));
        assert!(desktop.contains(&format!("Icon={BINARY_NAME}")));
        // appimagetool resolves the entry at the AppDir root.
        let root_entry = fs::read_to_string(appdir.join(format!("{BINARY_NAME}.desktop")))
            .expect("root desktop entry");
        assert!(root_entry.contains(&format!("Name={APP_NAME}")));

        for (size, _) in ICON_SIZES {
            assert!(
                appdir
                    .join("usr")
                    .join("share")
                    .join("icons")
                    .join("hicolor")
                    .join(size)
                    .join("apps")
                    .join(format!("{BINARY_NAME}.png"))
                    .is_file(),
                "missing hicolor {size} icon"
            );
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(appdir.join("AppRun"))
                .expect("AppRun stat")
                .permissions()
                .mode();
            assert!(mode & 0o111 != 0, "AppRun must be executable");
        }

        // AppRun must reference the real binary path.
        let apprun = fs::read_to_string(appdir.join("AppRun")).expect("AppRun");
        assert!(apprun.contains(&format!("usr/bin/{BINARY_NAME}")));
    }

    #[test]
    fn stage_appdir_fails_clearly_without_the_release_binary() {
        let root = temp_root("nobinary");
        real_icons(&root);
        let appdir = root.join("out").join(format!("{BINARY_NAME}.AppDir"));
        let err = stage_appdir(&root, &appdir).expect_err("must fail without the binary");
        assert!(err.to_string().contains("release binary not found"));
    }
}
