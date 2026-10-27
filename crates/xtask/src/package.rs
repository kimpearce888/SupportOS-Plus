//! Desktop packaging — `xtask package` (AppImage) and `xtask deb`.
//!
//! The Tauri bundler and its msi/nsis/dmg/app targets left with the Phase 4
//! cutover. Linux desktop distribution ships two install-ready packages,
//! both staged from the same release binary and checked-in icon set:
//!
//! * a type-2 **AppImage** — self-contained, runs without installation;
//! * a standard **.deb** — declares the GUI toolkit's runtime library set
//!   so APT installs everything a desktop session needs.
//!
//! Packaging stays in testable halves:
//!
//! * [`stage_appdir`] / [`stage_deb`] — build the staging trees on disk;
//!   unit-tested directly.
//! * [`package`] / [`package_deb`] — release build + staging + running
//!   `appimagetool` (located via `APPIMAGETOOL`, cached under
//!   `target/tools/`, or downloaded from the version-pinned release and
//!   SHA-256-verified before execution — item 158; run with
//!   `APPIMAGE_EXTRACT_AND_RUN=1` so it works on FUSE-less builders) /
//!   `dpkg-deb --root-owner-group`.
//!
//! Naming layers (maintainer instruction, 2026-10-19; completed for
//! 1.0.0): `SupportOS+` is the human display name (window title, desktop
//! entry `Name`, release title); `SupportOS-Plus` is the product
//! identifier (repository, window `app_id`/`StartupWMClass`, artifact
//! base name); `supportos-plus` is the Debian package name — and, since
//! 1.0.0, also the binary and crate identifier (Cargo names cannot carry
//! `+`, and the hyphenated form matches the package name).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Human-facing display name (window title / desktop entry Name).
pub const DISPLAY_NAME: &str = "SupportOS+";

/// Product identifier: repository, artifacts, `app_id`, `StartupWMClass`.
pub const PRODUCT_ID: &str = "SupportOS-Plus";

/// Debian package name (lowercase identifier; `+` is not allowed).
pub const DEB_PACKAGE: &str = "supportos-plus";

/// The app-shell binary name (crates/app-shell `[[bin]]`). Matches the
/// Debian package name: installing `supportos-plus` provides the
/// `supportos-plus` command.
pub const BINARY_NAME: &str = "supportos-plus";

/// Workspace package version (xtask inherits the workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Repository homepage, used verbatim in the .deb control file.
const HOMEPAGE: &str = "https://github.com/kimpearce888/SupportOS-Plus";

/// Icon sizes shipped into the hicolor tree.
const ICON_SIZES: &[(&str, &str)] = &[
    ("32x32", "32x32.png"),
    ("128x128", "128x128.png"),
    ("256x256", "256x256.png"),
    ("1024x1024", "1024x1024.png"),
];

/// The pinned appimagetool release (clean-audit item 158). The URL used
/// to point at the moving `continuous` build with no integrity check —
/// whatever GitHub served that day was executed in the release path. The
/// version tag and SHA-256 are pinned instead: a mismatched tool (tampered
/// download, corrupted cache, upstream re-upload) refuses to run.
const APPIMAGETOOL_URL: &str = "https://github.com/AppImage/appimagetool/releases/download/\
                                1.9.1/appimagetool-x86_64.AppImage";
const APPIMAGETOOL_SHA256: &str =
    "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0";

/// Host libraries that must ship inside the AppImage. winit loads the
/// X11 keyboard stack via `dlopen` at window creation
/// (`xkbcommon-dl`), and not every Linux host ships the X11 flavor
/// (GUI-less servers, runners, containers), which would leave the
/// packaged app unable to open a window on exactly those machines the
/// AppImage exists for. The trio is MIT/X11-licensed, glibc-free and a
/// few hundred KiB — safe and cheap to redistribute inside the image.
/// Found on the build host via `ldconfig -p` and copied into
/// `usr/lib`; `AppRun` puts that directory on `LD_LIBRARY_PATH`.
const BUNDLED_HOST_LIBS: &[&str] = &[
    "libxkbcommon.so.0",
    "libxkbcommon-x11.so.0",
    "libxcb-xkb.so.1",
];

/// dpkg architecture for the current build host.
///
/// # Errors
/// Returns an error on architectures with no mapping (only x86_64 and
/// aarch64 hosts are packaged today).
fn deb_arch() -> anyhow::Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        other => anyhow::bail!("no Debian architecture mapped for {other}"),
    }
}

/// AppImage architecture for a Rust target arch, using the appimagetool
/// `ARCH` convention and the artifact-name suffix (the official tool
/// publishes `-x86_64` and `-aarch64` images).
///
/// # Errors
/// Returns an error on architectures with no mapping (only x86_64 and
/// aarch64 hosts are packaged today — the same set [`deb_arch`] maps).
fn appimage_arch_for(arch: &str) -> anyhow::Result<&'static str> {
    match arch {
        "x86_64" => Ok("x86_64"),
        "aarch64" => Ok("aarch64"),
        other => anyhow::bail!("no AppImage architecture mapped for {other}"),
    }
}

/// The AppImage architecture of the current build host.
fn appimage_arch() -> anyhow::Result<&'static str> {
    appimage_arch_for(std::env::consts::ARCH)
}

/// The GUI stack loads its windowing and graphics libraries at runtime
/// (`winit`'s X11 backends, `xkbcommon`, `glutin`/`glow`, wgpu's GLES
/// fallback and the Wayland client all `dlopen` their system libraries),
/// so `ldd` cannot see them. The .deb therefore declares the
/// client-library set explicitly and APT pulls it in on minimal
/// systems; the GL driver itself stays host-provided (Mesa software
/// rendering works on GPU-less machines).
const DEB_DEPENDS: &str = "libc6 (>= 2.35), libgcc-s1, libx11-6, libxext6, libxfixes3, \
                           libxrender1, libxcursor1, libxrandr2, libxi6, libxinerama1, \
                           libxkbcommon0, libxkbcommon-x11-0, libxcb1, libgl1, libegl1, \
                           libgles2, libwayland-client0, libwayland-cursor0, libwayland-egl1";

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

/// Everything `deb` needs.
pub struct DebOptions {
    /// Workspace root (the directory of the top-level `Cargo.toml`).
    pub workspace_root: PathBuf,
    /// Where the resulting .deb lands.
    pub out_dir: PathBuf,
    /// Extra arguments forwarded to the `cargo build` invocation.
    pub extra_cargo_args: Vec<String>,
    /// Skips the cargo build (already built, e.g. CI re-runs).
    pub skip_build: bool,
}

/// Release build shared by both package targets. The shipped artifacts
/// must carry the pdf/zip parse sandbox (Landlock + seccomp, audit item
/// 40): the feature is opt-in at the Cargo level, so packaging enables it
/// explicitly. It only exists on Linux targets, and both packages are
/// Linux-only.
fn release_build(workspace_root: &Path, extra_args: &[String]) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    // `--locked` (audit item 165): the release path builds exactly the
    // committed Cargo.lock — a silently rewritten lock (stale lock vs
    // drifted manifests) would ship artifacts from unpinned versions.
    cmd.args([
        "build",
        "--release",
        "--locked",
        "-p",
        "supportos-plus-app-shell",
    ])
    .current_dir(workspace_root);
    if cfg!(target_os = "linux") {
        cmd.args(["--features", "supportos-plus-core/sandbox"]);
    }
    for arg in extra_args {
        cmd.arg(arg);
    }
    let status = cmd.status()?;
    anyhow::ensure!(status.success(), "release build failed");
    Ok(())
}

/// Build, stage, and pack the AppImage. Returns the artifact path.
pub fn package(opts: PackageOptions) -> anyhow::Result<PathBuf> {
    if !opts.skip_build {
        release_build(&opts.workspace_root, &opts.extra_cargo_args)?;
    }

    // The checked-in icon set is packaging's contract: verify the full
    // set (PNG dimensions, ICO/ICNS structure) before anything consumes
    // it (audit item 164 — `verify-icons` had no automation caller).
    verify_icon_set(&opts.workspace_root)?;

    let appdir = opts.out_dir.join(format!("{BINARY_NAME}.AppDir"));
    stage_appdir(&opts.workspace_root, &appdir)?;
    ensure_host_libs_bundled(&appdir)?;

    let tool = resolve_appimagetool(&opts.workspace_root, opts.appimagetool.as_deref())?;
    // Audit item 159: the AppImage lane used to hardcode `x86_64` in both
    // the artifact name and the tool's ARCH env while the .deb lane
    // followed the host (`deb_arch`) — an aarch64 host shipped an arm64
    // .deb next to an AppImage labeled (and runtime-pinned) x86_64. The
    // artifact now follows the build host exactly like the deb.
    let arch = appimage_arch()?;
    let output = opts.out_dir.join(format!("{PRODUCT_ID}-{arch}.AppImage"));
    if output.exists() {
        fs::remove_file(&output)?;
    }

    // ARCH pins the runtime; EXTRACT_AND_RUN lets the tool itself run on
    // FUSE-less builders; NO_STRIP keeps the appimage working under
    // `strip = true` release profiles.
    let status = Command::new(&tool)
        .arg(&appdir)
        .arg(&output)
        .env("ARCH", arch)
        .env("APPIMAGE_EXTRACT_AND_RUN", "1")
        .env("NO_STRIP", "1")
        .status()?;
    anyhow::ensure!(
        status.success() && output.exists(),
        "appimagetool failed (status {status})"
    );
    Ok(output)
}

/// Build, stage, and pack the .deb. Returns the artifact path.
pub fn package_deb(opts: DebOptions) -> anyhow::Result<PathBuf> {
    if !opts.skip_build {
        release_build(&opts.workspace_root, &opts.extra_cargo_args)?;
    }

    // Same icon contract as the AppImage lane (audit item 164).
    verify_icon_set(&opts.workspace_root)?;

    let staging = opts.out_dir.join("debroot");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    stage_deb(&opts.workspace_root, &staging)?;

    let output = opts
        .out_dir
        .join(format!("{DEB_PACKAGE}_{VERSION}_{}.deb", deb_arch()?));
    if output.exists() {
        fs::remove_file(&output)?;
    }
    // `--root-owner-group` stamps the payload root:root, which is what an
    // installed system expects; `dpkg-deb` ships with the build OS.
    let status = Command::new("dpkg-deb")
        .arg("--root-owner-group")
        .arg("--build")
        .arg(&staging)
        .arg(&output)
        .status()?;
    anyhow::ensure!(
        status.success() && output.is_file(),
        "dpkg-deb failed (status {status})"
    );
    Ok(output)
}

/// The desktop entry text shared by both packages; `exec` is either the
/// AppDir-relative path (AppImage) or the absolute install path (.deb).
fn desktop_entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={DISPLAY_NAME}\n\
         Comment=Offline-first Help Scout support desk\n\
         Exec={exec}\n\
         Icon={BINARY_NAME}\n\
         Terminal=false\n\
         Categories=Office;Utility;\n\
         StartupWMClass={PRODUCT_ID}\n"
    )
}

/// Copy the checked-in icon set into a hicolor tree root.
fn copy_hicolor_icons(icons_dir: &Path, hicolor_root: &Path) -> anyhow::Result<()> {
    for (size, file) in ICON_SIZES {
        let src = icons_dir.join(file);
        anyhow::ensure!(
            src.is_file(),
            "missing icon {} — run `cargo xtask verify-icons`",
            src.display()
        );
        let dst_dir = hicolor_root.join(size).join("apps");
        fs::create_dir_all(&dst_dir)?;
        fs::copy(&src, dst_dir.join(format!("{BINARY_NAME}.png")))?;
    }
    Ok(())
}

/// Stage the AppDir: binary, desktop entry, icons, AppRun.
///
/// Layout:
///
/// ```text
/// supportos-plus.AppDir/
/// ├── AppRun                          (sh shim, execs usr/bin/…)
/// ├── supportos-plus.png              (root icon)
/// ├── .DirIcon                        -> root icon
/// ├── usr/bin/supportos-plus
/// ├── usr/share/applications/supportos-plus.desktop
/// └── usr/share/icons/hicolor/<size>/apps/supportos-plus.png
/// ```
pub fn stage_appdir(workspace_root: &Path, appdir: &Path) -> anyhow::Result<()> {
    // Clean-audit item 157: a leftover staging tree from a previous build
    // must not ship — the .deb path already wipes `debroot` the same way.
    // Without this, files that are no longer part of the layout (a
    // renamed artifact, a removed icon size, a stale AppRun) would ride
    // along inside the packed AppImage forever.
    if appdir.exists() {
        fs::remove_dir_all(appdir)?;
    }
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
    let desktop = desktop_entry(&format!("usr/bin/{BINARY_NAME}"));
    fs::write(apps_dir.join(format!("{BINARY_NAME}.desktop")), &desktop)?;
    // appimagetool looks the desktop entry up at the AppDir root; keep the
    // hicolor copy too so the layout follows both conventions.
    fs::write(appdir.join(format!("{BINARY_NAME}.desktop")), desktop)?;

    copy_hicolor_icons(&icons_dir, &hicolor)?;

    // Root icon + .DirIcon (what file managers show).
    let root_icon = appdir.join(format!("{BINARY_NAME}.png"));
    fs::copy(icons_dir.join("256x256.png"), &root_icon)?;
    let dir_icon = appdir.join(".DirIcon");
    copy_or_link(&root_icon, &dir_icon)?;

    // AppRun shim: resolve the real AppDir even when symlinked from
    // $HOME/.local/bin after extraction; put the bundled keyboard libs
    // ahead of the host lookup path.
    let apprun = format!(
        "#!/bin/sh\n\
         HERE=\"$(dirname \"$(readlink -f \"$0\")\")\"\n\
         export LD_LIBRARY_PATH=\"$HERE/usr/lib${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}\"\n\
         exec \"$HERE/usr/bin/{BINARY_NAME}\" \"$@\"\n"
    );
    let apprun_path = appdir.join("AppRun");
    fs::write(&apprun_path, apprun)?;
    make_executable(&apprun_path)?;

    Ok(())
}

/// Stage the .deb package tree: control metadata + the install payload.
///
/// Layout:
///
/// ```text
/// debroot/
/// ├── DEBIAN/control
/// └── usr/
///     ├── bin/supportos-plus
///     └── share/
///         ├── applications/supportos-plus.desktop
///         └── icons/hicolor/<size>/apps/supportos-plus.png
/// ```
///
/// No maintainer scripts and no triggers file: the desktop-file and
/// hicolor caches are maintained by their system trigger providers
/// (`desktop-file-utils`, hicolor theme packages), and user data lives
/// under `XDG_DATA_HOME` where the package never owns it.
pub fn stage_deb(workspace_root: &Path, staging: &Path) -> anyhow::Result<()> {
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

    let bin_dst = staging.join("usr").join("bin").join(BINARY_NAME);
    fs::create_dir_all(bin_dst.parent().unwrap())?;
    fs::copy(&bin_src, &bin_dst)?;
    make_executable(&bin_dst)?;

    let desktop = desktop_entry(&format!("/usr/bin/{BINARY_NAME}"));
    let apps_dir = staging.join("usr").join("share").join("applications");
    fs::create_dir_all(&apps_dir)?;
    fs::write(apps_dir.join(format!("{BINARY_NAME}.desktop")), &desktop)?;

    // Audit §4 hygiene: Debian policy expects every package to ship
    // usr/share/doc/<pkg>/copyright; the .deb previously installed none.
    // Machine-readable format, source pointing at the shipped LICENSE.
    let doc_dir = staging
        .join("usr")
        .join("share")
        .join("doc")
        .join(DEB_PACKAGE);
    fs::create_dir_all(&doc_dir)?;
    fs::write(
        doc_dir.join("copyright"),
        format!(
            "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
             Upstream-Name: SupportOS+\n\
             Source: {HOMEPAGE}\n\
             \n\
             Files: *\n\
             Copyright: 2026 kimpearce888\n\
             License: MIT\n\
              Permission is hereby granted, free of charge, to any person\n\
              obtaining a copy of this software and associated documentation\n\
              files (the \"Software\"), to deal in the Software without\n\
              restriction, including without limitation the rights to use,\n\
              copy, modify, merge, publish, distribute, sublicense, and/or\n\
              sell copies of the Software, and to permit persons to whom the\n\
              Software is furnished to do so, subject to the following\n\
              conditions: The above copyright notice and this permission\n\
              notice shall be included in all copies or substantial portions\n\
              of the Software. THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT\n\
              WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT\n\
              LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A\n\
              PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\n\
              AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES\n\
              OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR\n\
              OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE\n\
              SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\n",
        ),
    )?;

    let hicolor = staging
        .join("usr")
        .join("share")
        .join("icons")
        .join("hicolor");
    copy_hicolor_icons(&icons_dir, &hicolor)?;

    let control = format!(
        "Package: {DEB_PACKAGE}\n\
         Version: {VERSION}\n\
         Section: utils\n\
         Priority: optional\n\
         Architecture: {}\n\
         Maintainer: Kim Pearce <kimpearce888@users.noreply.github.com>\n\
         Installed-Size: {}\n\
         Depends: {DEB_DEPENDS}\n\
         Homepage: {HOMEPAGE}\n\
         Description: {}\n",
        deb_arch()?,
        installed_size_kib(&staging.join("usr"))?,
        deb_description(),
    );
    let debian_dir = staging.join("DEBIAN");
    fs::create_dir_all(&debian_dir)?;
    fs::write(debian_dir.join("control"), control)?;

    // Debian payload hygiene: standard modes (dirs 0755, files 0644, the
    // binary 0755). The build user's umask (e.g. 002 in CI containers)
    // must not leak into the installed package.
    normalize_payload_modes(staging)?;
    make_executable(&bin_dst)?;
    Ok(())
}

/// Stamp standard Debian modes over a staging tree: directories 0755,
/// files 0644. Callers re-apply the executable bit where required.
fn normalize_payload_modes(root: &Path) -> anyhow::Result<()> {
    fn walk(dir: &Path) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                set_mode(&path, 0o755)?;
                walk(&path)?;
            } else {
                set_mode(&path, 0o644)?;
            }
        }
        Ok(())
    }
    anyhow::ensure!(root.is_dir(), "staging tree missing: {}", root.display());
    walk(root)?;
    Ok(())
}

/// Debian control `Description`: synopsis line, then one-space-indented
/// continuation lines (policy format). Built explicitly because Rust's
/// `\n\` literal continuation eats leading whitespace, and the required
/// one-space indentation is exactly what would be eaten.
fn deb_description() -> String {
    let mut out = String::from("SupportOS+ offline-first support desk");
    for line in [
        " SupportOS-Plus is a local-first Help Scout-style support desk:",
        " unified inbox, knowledge base, semantic docs search, SLA",
        " reporting and local-only AI assistants. All data stays on",
        " this machine.",
    ] {
        out.push('\n');
        out.push_str(line);
    }
    out
}

/// Installed-Size control field: the payload subtree's size in KiB.
fn installed_size_kib(root: &Path) -> anyhow::Result<u64> {
    fn walk(dir: &Path, total: &mut u64) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                walk(&entry.path(), total)?;
            } else {
                *total += meta.len();
            }
        }
        Ok(())
    }
    anyhow::ensure!(root.is_dir(), "staging payload missing: {}", root.display());
    let mut total = 0u64;
    walk(root, &mut total)?;
    Ok(total.div_ceil(1024))
}

fn copy_or_link(src: &Path, dst: &Path) -> anyhow::Result<()> {
    // A plain copy keeps the AppDir self-contained when it is moved
    // around; hard-link/symlink would break for `cp -r` consumers.
    fs::copy(src, dst)?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(mode);
    fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> std::io::Result<()> {
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
    anyhow::bail!("desktop packaging is Linux-only")
}

/// Parse an `ldconfig -p` cache and find the exact library name.
/// Cache lines look like `\t<name> (flags) => <path>`; only the first
/// token is the library name, and the entry is only valid when the
/// path exists on disk.
fn find_lib_in_ldconfig(cache: &str, name: &str) -> Option<PathBuf> {
    cache.lines().find_map(|line| {
        let (key, path) = line.split_once("=>")?;
        let cached_name = key.split_whitespace().next()?;
        if cached_name != name {
            return None;
        }
        let path = path.trim();
        Path::new(path).is_file().then(|| PathBuf::from(path))
    })
}

/// Copy [`BUNDLED_HOST_LIBS`] from the build host into the AppDir so
/// the AppImage is self-contained on GUI-less hosts.
///
/// # Errors
/// Fails with an apt-oriented hint when the build host itself lacks a
/// library — shipping an AppImage that cannot open a window would be
/// worse than failing the build.
fn ensure_host_libs_bundled(appdir: &Path) -> anyhow::Result<()> {
    let out = Command::new("ldconfig")
        .arg("-p")
        .output()
        .map_err(|e| anyhow::anyhow!("cannot run ldconfig -p: {e}"))?;
    anyhow::ensure!(out.status.success(), "ldconfig -p failed");
    let cache = String::from_utf8_lossy(&out.stdout);
    let lib_dir = appdir.join("usr").join("lib");
    fs::create_dir_all(&lib_dir)?;
    for lib in BUNDLED_HOST_LIBS {
        let path = find_lib_in_ldconfig(&cache, lib).ok_or_else(|| {
            anyhow::anyhow!(
                "host library {lib} not found — install \
                 libxkbcommon0 libxkbcommon-x11-0 libxcb-xkb1 on the build host \
                 so the AppImage can bundle them"
            )
        })?;
        fs::copy(&path, lib_dir.join(lib))?;
    }
    Ok(())
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
    if cached.is_file() && verify_appimagetool(&cached).is_ok() {
        return Ok(cached);
    }
    download_appimagetool(&cached)?;
    verify_appimagetool(&cached)?;
    Ok(cached)
}

/// SHA-256 of the tool on disk (streamed; the AppImage is ~15 MB).
fn sha256_file(path: &Path) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Refuse to execute a tool that does not match the pinned checksum.
fn verify_appimagetool(path: &Path) -> anyhow::Result<()> {
    let got = sha256_file(path)?;
    anyhow::ensure!(
        got == APPIMAGETOOL_SHA256,
        "appimagetool checksum mismatch at {} — expected {APPIMAGETOOL_SHA256}, got {got}; \
         refusing to execute an unverified tool in the release path",
        path.display()
    );
    Ok(())
}

/// Verify the checked-in icon set that both packages stage. Kept here so
/// the packaging entry points own the contract (audit item 164): PNG
/// payloads must be real, correctly sized images and the ICO/ICNS
/// containers must carry the full declared size set.
fn verify_icon_set(workspace_root: &Path) -> anyhow::Result<()> {
    let shell_dir = workspace_root.join("crates").join("app-shell");
    crate::icons::verify_icons(&shell_dir)
        .map_err(|e| anyhow::anyhow!("icon set failed verification: {e}"))
}

fn download_appimagetool(dest: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    eprintln!("package: downloading appimagetool 1.9.1 (sha256 {APPIMAGETOOL_SHA256})…");
    // Bounded download (audit item 160): a default blocking GET has no
    // timeout, so a stalled connection could hang a CI job forever.
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()?;
    let bytes = client
        .get(APPIMAGETOOL_URL)
        .send()?
        .error_for_status()?
        .bytes()?;
    // Land the tool at its final path atomically (audit item 160):
    // write + fsync a `.part` sibling, then rename. A crash mid-download
    // used to leave a truncated file at the cached path itself, which
    // `resolve_appimagetool` trusted on every later run — the package
    // step then failed with a confusing "cannot execute" error.
    install_tool_bytes(dest, &bytes).inspect_err(|_| {
        let _ = fs::remove_file(dest.with_extension("part"));
    })?;
    eprintln!("package: appimagetool cached at {}", dest.display());
    Ok(())
}

/// Write the tool payload to `dest` atomically: temp sibling + fsync +
/// rename, executable bit set before the rename so the cached tool is
/// runnable the moment it exists.
fn install_tool_bytes(dest: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = dest.with_extension("part");
    {
        use std::io::Write as _;
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    make_executable(&tmp)?;
    fs::rename(&tmp, dest)?;
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
        assert!(desktop.contains(&format!("Name={DISPLAY_NAME}")));
        assert!(desktop.contains(&format!("Exec=usr/bin/{BINARY_NAME}")));
        assert!(desktop.contains(&format!("Icon={BINARY_NAME}")));
        assert!(desktop.contains(&format!("StartupWMClass={PRODUCT_ID}")));
        // appimagetool resolves the entry at the AppDir root.
        let root_entry = fs::read_to_string(appdir.join(format!("{BINARY_NAME}.desktop")))
            .expect("root desktop entry");
        assert!(root_entry.contains(&format!("Name={DISPLAY_NAME}")));

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

        // AppRun must reference the real binary path and prepend the
        // bundled library directory.
        let apprun = fs::read_to_string(appdir.join("AppRun")).expect("AppRun");
        assert!(apprun.contains(&format!("usr/bin/{BINARY_NAME}")));
        assert!(apprun.contains("export LD_LIBRARY_PATH=\"$HERE/usr/lib"));
    }

    /// Clean-audit item 157: staging wipes a leftover AppDir first, so
    /// files that are no longer part of the layout cannot ship.
    #[test]
    fn stage_appdir_cleans_a_stale_staging_tree() {
        let root = temp_root("stale");
        fake_release_build(&root);
        real_icons(&root);

        let appdir = root.join("out").join(format!("{BINARY_NAME}.AppDir"));
        let stale = appdir.join("usr").join("bin").join("removed-artifact.old");
        fs::create_dir_all(appdir.join("usr").join("bin")).expect("stale tree");
        fs::write(&stale, b"stale build output").expect("stale file");

        stage_appdir(&root, &appdir).expect("stage");

        assert!(
            !stale.exists(),
            "the stale file must not survive the re-stage"
        );
        assert!(
            appdir.join("usr").join("bin").join(BINARY_NAME).is_file(),
            "the fresh layout is staged"
        );
    }

    #[test]
    fn ldconfig_parse_matches_exact_library_names_only() {
        let root = temp_root("ldcache");
        let lib_a = root.join("libxkbcommon.so.0");
        fs::write(&lib_a, b"fake lib").expect("fake lib file");
        let cache = format!(
            "\tlibxkbcommon.so (libc6,x86-64) => /nowhere/libxkbcommon.so\n\
             \tlibxkbcommon.so.0 (libc6,x86-64) => {}\n\
             \tlibxkbcommon-x11.so.0 (libc6,x86-64) => /nowhere/libxkbcommon-x11.so.0\n",
            lib_a.display()
        );
        let found = find_lib_in_ldconfig(&cache, "libxkbcommon.so.0").expect("exact match");
        assert_eq!(found, lib_a);
        // No fuzzy prefix matching and no hits for entries whose path
        // does not exist on disk.
        assert!(find_lib_in_ldconfig(&cache, "libxkbcommon.so").is_none());
        assert!(find_lib_in_ldconfig(&cache, "libxkbcommon-x11.so.0").is_none());
    }

    #[test]
    fn stage_appdir_fails_clearly_without_the_release_binary() {
        let root = temp_root("nobinary");
        real_icons(&root);
        let appdir = root.join("out").join(format!("{BINARY_NAME}.AppDir"));
        let err = stage_appdir(&root, &appdir).expect_err("must fail without the binary");
        assert!(err.to_string().contains("release binary not found"));
    }

    #[test]
    fn stage_deb_builds_the_full_layout() {
        let root = temp_root("deb-layout");
        fake_release_build(&root);
        real_icons(&root);

        let staging = root.join("out").join("debroot");
        stage_deb(&root, &staging).expect("stage deb");

        // Control metadata.
        let control = fs::read_to_string(staging.join("DEBIAN").join("control")).expect("control");
        assert!(control.contains(&format!("Package: {DEB_PACKAGE}")));
        assert!(control.contains(&format!("Version: {VERSION}")));
        assert!(control.contains("Priority: optional"));
        assert!(control.lines().any(|l| l.starts_with("Architecture: ")));
        assert!(control.contains(&format!("Homepage: {HOMEPAGE}")));
        assert!(control.contains("Description: SupportOS+ offline-first support desk"));
        // Debian policy: description continuation lines are one-space
        // indented — dpkg-deb refuses to parse the control otherwise.
        assert!(control.contains("\n SupportOS-Plus is a local-first"));
        // The GUI client-library set must be declared for APT.
        assert!(control.contains("Depends:"));
        assert!(control.contains("libxkbcommon0"));
        assert!(control.contains("libgl1"));
        // v1.1.1: the GLES client library is part of the declared set —
        // wgpu's GL fallback dlopens it, and 1.1.0 died on machines that
        // had neither a Vulkan ICD nor libGLESv2.
        assert!(control.contains("libgles2"));
        assert!(control.contains("libc6 (>= 2.35)"));
        let installed_size = control
            .lines()
            .find_map(|l| l.strip_prefix("Installed-Size: "))
            .expect("Installed-Size line")
            .parse::<u64>()
            .expect("numeric Installed-Size");
        assert!(
            installed_size > 0,
            "Installed-Size must reflect the payload"
        );

        // Payload: absolute Exec path, full hicolor set, executable binary.
        let desktop_path = staging
            .join("usr")
            .join("share")
            .join("applications")
            .join(format!("{BINARY_NAME}.desktop"));
        let desktop = fs::read_to_string(&desktop_path).expect("desktop entry");
        assert!(desktop.contains(&format!("Exec=/usr/bin/{BINARY_NAME}")));
        assert!(desktop.contains(&format!("Name={DISPLAY_NAME}")));
        assert!(desktop.contains(&format!("StartupWMClass={PRODUCT_ID}")));
        assert!(staging.join("usr").join("bin").join(BINARY_NAME).is_file());
        for (size, _) in ICON_SIZES {
            assert!(
                staging
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

        // No maintainer scripts: caches are owned by system trigger
        // providers and user data is never owned by the package.
        assert!(!staging.join("DEBIAN").join("postinst").exists());
        assert!(!staging.join("DEBIAN").join("triggers").exists());

        // Standard Debian modes, whatever the build umask is.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&staging.join("DEBIAN").join("control")), 0o644);
            assert_eq!(mode(&desktop_path), 0o644);
            assert_eq!(
                mode(&staging.join("usr").join("bin").join(BINARY_NAME)),
                0o755
            );
            assert_eq!(mode(&staging.join("usr").join("bin")), 0o755);
        }
    }

    #[test]
    fn stage_deb_fails_clearly_without_the_release_binary() {
        let root = temp_root("deb-nobinary");
        real_icons(&root);
        let staging = root.join("out").join("debroot");
        let err = stage_deb(&root, &staging).expect_err("must fail without the binary");
        assert!(err.to_string().contains("release binary not found"));
    }

    #[test]
    fn package_names_follow_the_naming_layers() {
        // Display vs identifier vs package/binary names stay aligned with
        // the maintainer instruction: SupportOS+ for humans,
        // SupportOS-Plus for identifiers, Debian-safe names for dpkg.
        assert_eq!(DISPLAY_NAME, "SupportOS+");
        assert_eq!(PRODUCT_ID, "SupportOS-Plus");
        assert_eq!(DEB_PACKAGE, "supportos-plus");
        assert!(!DEB_PACKAGE.contains('+'), "dpkg names cannot carry +");
        assert!(!PRODUCT_ID.contains('+'), "artifact names stay URL-safe");
    }

    #[test]
    fn appimage_arch_covers_the_same_hosts_as_the_deb_lane() {
        // Audit item 159: the AppImage lane must map exactly the host set
        // the .deb lane already maps — x86_64 and aarch64 — and refuse
        // anything else instead of silently mislabeling the artifact.
        assert_eq!(appimage_arch_for("x86_64").expect("x86_64"), "x86_64");
        assert_eq!(appimage_arch_for("aarch64").expect("aarch64"), "aarch64");
        let err = appimage_arch_for("riscv64").expect_err("unmapped arch");
        assert!(err.to_string().contains("no AppImage architecture mapped"));
        // The host mapping agrees with the compile-time target.
        assert_eq!(
            appimage_arch().expect("host arch"),
            appimage_arch_for(std::env::consts::ARCH).expect("host arch")
        );
    }

    #[test]
    fn tool_cache_lands_atomically_at_the_final_path() {
        // Audit item 160: the cached tool must never exist in partial
        // form — write + fsync + rename leaves either the absent path or
        // the complete, executable file, and never a `.part` sibling.
        let root = temp_root("atomic-tool");
        let dest = root.join("tools").join("appimagetool-x86_64.AppImage");
        fs::create_dir_all(dest.parent().unwrap()).expect("tools dir");

        install_tool_bytes(&dest, b"first").expect("first install");
        assert_eq!(fs::read(&dest).expect("read dest"), b"first");
        assert!(
            !dest.with_extension("part").exists(),
            "no temp sibling left"
        );

        // A re-install replaces the file in place (the download-retry
        // path), still without a temp leftover.
        install_tool_bytes(&dest, b"second-and-longer").expect("re-install");
        assert_eq!(fs::read(&dest).expect("read dest"), b"second-and-longer");
        assert!(!dest.with_extension("part").exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&dest).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o755, "cached tool must be executable");
        }
    }
}
