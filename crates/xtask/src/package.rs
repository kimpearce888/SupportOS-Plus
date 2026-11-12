use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const DISPLAY_NAME: &str = "SupportOS+";

pub const PRODUCT_ID: &str = "SupportOS-Plus";

pub const DEB_PACKAGE: &str = "supportos-plus";

pub const BINARY_NAME: &str = "supportos-plus";

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const HOMEPAGE: &str = "https://github.com/kimpearce888/SupportOS-Plus";

const ICON_SIZES: &[(&str, &str)] = &[
    ("32x32", "32x32.png"),
    ("128x128", "128x128.png"),
    ("256x256", "256x256.png"),
    ("1024x1024", "1024x1024.png"),
];

const APPIMAGETOOL_PINS: &[(&str, &str, &str)] = &[
    (
        "x86_64",
        "https://github.com/AppImage/appimagetool/releases/download/\
                                1.9.1/appimagetool-x86_64.AppImage",
        "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0",
    ),
    (
        "aarch64",
        "https://github.com/AppImage/appimagetool/releases/download/\
                                1.9.1/appimagetool-aarch64.AppImage",
        "f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158",
    ),
];
const APPIMAGE_RUNTIMES: &[(&str, &str, &str)] = &[
    (
        "x86_64",
        "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64",
        "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d",
    ),
    (
        "aarch64",
        "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-aarch64",
        "00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444",
    ),
];
const APPIMAGETOOL_RETRY_BACKOFF_SECS: &[u64] = &[5, 15, 45, 90];

const BUNDLED_HOST_LIBS: &[&str] = &[
    "libxkbcommon.so.0",
    "libxkbcommon-x11.so.0",
    "libxcb-xkb.so.1",
];

fn deb_arch() -> anyhow::Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        other => anyhow::bail!("no Debian architecture mapped for {other}"),
    }
}

fn appimage_arch_for(arch: &str) -> anyhow::Result<&'static str> {
    match arch {
        "x86_64" => Ok("x86_64"),
        "aarch64" => Ok("aarch64"),
        other => anyhow::bail!("no AppImage architecture mapped for {other}"),
    }
}

fn appimage_arch() -> anyhow::Result<&'static str> {
    appimage_arch_for(std::env::consts::ARCH)
}

const DEB_DEPENDS: &str = "libc6 (>= 2.35), libgcc-s1, libx11-6, libxext6, libxfixes3, \
                           libxrender1, libxcursor1, libxrandr2, libxi6, libxinerama1, \
                           libxkbcommon0, libxkbcommon-x11-0, libxcb1, libgl1, libegl1, \
                           libgles2, libwayland-client0, libwayland-cursor0, libwayland-egl1";

pub struct PackageOptions {
    pub workspace_root: PathBuf,
    pub out_dir: PathBuf,
    pub appimagetool: Option<PathBuf>,
    pub extra_cargo_args: Vec<String>,
    pub skip_build: bool,
}

pub struct DebOptions {
    pub workspace_root: PathBuf,
    pub out_dir: PathBuf,
    pub extra_cargo_args: Vec<String>,
    pub skip_build: bool,
}

fn release_build(workspace_root: &Path, extra_args: &[String]) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
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

pub fn package(opts: PackageOptions) -> anyhow::Result<PathBuf> {
    if !opts.skip_build {
        release_build(&opts.workspace_root, &opts.extra_cargo_args)?;
    }

    verify_icon_set(&opts.workspace_root)?;

    let appdir = opts.out_dir.join(format!("{BINARY_NAME}.AppDir"));
    stage_appdir(&opts.workspace_root, &appdir)?;
    ensure_host_libs_bundled(&appdir)?;

    let tool = resolve_appimagetool(&opts.workspace_root, opts.appimagetool.as_deref())?;
    let arch = appimage_arch()?;
    let runtime = resolve_appimage_runtime(&opts.workspace_root, arch)?;
    let output = opts.out_dir.join(format!("{PRODUCT_ID}-{arch}.AppImage"));
    if output.exists() {
        fs::remove_file(&output)?;
    }

    let status = Command::new(&tool)
        .arg("--runtime-file")
        .arg(&runtime)
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

pub fn package_deb(opts: DebOptions) -> anyhow::Result<PathBuf> {
    if !opts.skip_build {
        release_build(&opts.workspace_root, &opts.extra_cargo_args)?;
    }

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

pub fn stage_appdir(workspace_root: &Path, appdir: &Path) -> anyhow::Result<()> {
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

    fs::copy(&bin_src, &bin_dst)?;
    make_executable(&bin_dst)?;

    let desktop = desktop_entry(&format!("usr/bin/{BINARY_NAME}"));
    fs::write(apps_dir.join(format!("{BINARY_NAME}.desktop")), &desktop)?;
    fs::write(appdir.join(format!("{BINARY_NAME}.desktop")), desktop)?;

    copy_hicolor_icons(&icons_dir, &hicolor)?;

    let root_icon = appdir.join(format!("{BINARY_NAME}.png"));
    fs::copy(icons_dir.join("256x256.png"), &root_icon)?;
    let dir_icon = appdir.join(".DirIcon");
    copy_or_link(&root_icon, &dir_icon)?;

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

    normalize_payload_modes(staging)?;
    make_executable(&bin_dst)?;
    Ok(())
}

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

fn resolve_appimagetool(
    workspace_root: &Path,
    override_path: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_path {
        anyhow::ensure!(path.is_file(), "APPIMAGETOOL not found: {}", path.display());
        return Ok(path.to_path_buf());
    }
    let arch = appimage_arch()?;
    let cached = workspace_root
        .join("target")
        .join("tools")
        .join(format!("appimagetool-{arch}.AppImage"));
    if cached.is_file() && verify_appimagetool(&cached, arch).is_ok() {
        return Ok(cached);
    }
    download_appimagetool(&cached, arch)?;
    verify_appimagetool(&cached, arch)?;
    Ok(cached)
}

fn appimage_runtime_for(arch: &str) -> anyhow::Result<(&'static str, &'static str)> {
    APPIMAGE_RUNTIMES
        .iter()
        .find(|(a, _, _)| *a == arch)
        .map(|(_, url, sha)| (*url, *sha))
        .ok_or_else(|| anyhow::anyhow!("no type-2 runtime pinned for arch {arch}"))
}

fn resolve_appimage_runtime(workspace_root: &Path, arch: &str) -> anyhow::Result<PathBuf> {
    let (url, sha) = appimage_runtime_for(arch)?;
    let cached = workspace_root
        .join("target")
        .join("tools")
        .join(format!("runtime-{arch}"));
    if cached.is_file() && verify_pinned_file(&cached, sha, "type-2 runtime").is_ok() {
        return Ok(cached);
    }
    download_pinned_asset(&cached, url, sha, "type-2 runtime")?;
    verify_pinned_file(&cached, sha, "type-2 runtime")?;
    Ok(cached)
}

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

fn verify_appimagetool(path: &Path, arch: &str) -> anyhow::Result<()> {
    let (_, sha) = appimagetool_pin_for(arch)?;
    verify_pinned_file(path, sha, "appimagetool")
}

fn appimagetool_pin_for(arch: &str) -> anyhow::Result<(&'static str, &'static str)> {
    APPIMAGETOOL_PINS
        .iter()
        .find(|(a, _, _)| *a == arch)
        .map(|(_, url, sha)| (*url, *sha))
        .ok_or_else(|| anyhow::anyhow!("no appimagetool pinned for arch {arch}"))
}

fn verify_pinned_file(path: &Path, expected: &str, label: &str) -> anyhow::Result<()> {
    let got = sha256_file(path)?;
    anyhow::ensure!(
        got == expected,
        "{label} checksum mismatch at {} — expected {expected}, got {got}; \
         refusing to use an unverified artifact in the release path",
        path.display()
    );
    Ok(())
}

fn verify_icon_set(workspace_root: &Path) -> anyhow::Result<()> {
    let shell_dir = workspace_root.join("crates").join("app-shell");
    crate::icons::verify_icons(&shell_dir)
        .map_err(|e| anyhow::anyhow!("icon set failed verification: {e}"))
}

fn download_appimagetool(dest: &Path, arch: &str) -> anyhow::Result<()> {
    let (url, sha) = appimagetool_pin_for(arch)?;
    download_pinned_asset(dest, url, sha, "appimagetool 1.9.1")
}

fn download_pinned_asset(dest: &Path, url: &str, sha256: &str, label: &str) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    eprintln!("package: downloading {label} (sha256 {sha256})…");
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()?;
    let bytes = fetch_pinned_with_retry(&client, url, label)?;
    install_tool_bytes(dest, &bytes).inspect_err(|_| {
        let _ = fs::remove_file(dest.with_extension("part"));
    })?;
    eprintln!("package: {label} cached at {}", dest.display());
    Ok(())
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

fn fetch_pinned_with_retry(
    client: &reqwest::blocking::Client,
    url: &str,
    label: &str,
) -> anyhow::Result<Vec<u8>> {
    let attempts = APPIMAGETOOL_RETRY_BACKOFF_SECS.len() + 1;
    let mut last_err = String::new();
    for attempt in 1..=attempts {
        match client.get(url).send() {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    match resp.bytes() {
                        Ok(body) => return Ok(body.to_vec()),
                        Err(e) => last_err = format!("response body read failed: {e}"),
                    }
                } else if is_retryable_status(status) {
                    last_err = format!("upstream returned HTTP {status}");
                } else {
                    anyhow::bail!(
                        "{label} download got HTTP {status}, which is not transient — \
                         refusing to retry the pinned URL {url}"
                    );
                }
            }
            Err(e) => last_err = format!("request failed: {e}"),
        }
        if attempt < attempts {
            let wait = APPIMAGETOOL_RETRY_BACKOFF_SECS[attempt - 1];
            eprintln!(
                "package: {label} download attempt {attempt}/{attempts} failed \
                 ({last_err}); retrying in {wait}s…"
            );
            std::thread::sleep(Duration::from_secs(wait));
        }
    }
    anyhow::bail!(
        "{label} download failed after {attempts} attempts: {last_err} \
         (pinned URL {url})"
    )
}

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

        let apprun = fs::read_to_string(appdir.join("AppRun")).expect("AppRun");
        assert!(apprun.contains(&format!("usr/bin/{BINARY_NAME}")));
        assert!(apprun.contains("export LD_LIBRARY_PATH=\"$HERE/usr/lib"));
    }

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

        let control = fs::read_to_string(staging.join("DEBIAN").join("control")).expect("control");
        assert!(control.contains(&format!("Package: {DEB_PACKAGE}")));
        assert!(control.contains(&format!("Version: {VERSION}")));
        assert!(control.contains("Priority: optional"));
        assert!(control.lines().any(|l| l.starts_with("Architecture: ")));
        assert!(control.contains(&format!("Homepage: {HOMEPAGE}")));
        assert!(control.contains("Description: SupportOS+ offline-first support desk"));
        assert!(control.contains("\n SupportOS-Plus is a local-first"));
        assert!(control.contains("Depends:"));
        assert!(control.contains("libxkbcommon0"));
        assert!(control.contains("libgl1"));
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

        assert!(!staging.join("DEBIAN").join("postinst").exists());
        assert!(!staging.join("DEBIAN").join("triggers").exists());

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
        assert_eq!(DISPLAY_NAME, "SupportOS+");
        assert_eq!(PRODUCT_ID, "SupportOS-Plus");
        assert_eq!(DEB_PACKAGE, "supportos-plus");
        assert!(!DEB_PACKAGE.contains('+'), "dpkg names cannot carry +");
        assert!(!PRODUCT_ID.contains('+'), "artifact names stay URL-safe");
    }

    #[test]
    fn appimage_arch_covers_the_same_hosts_as_the_deb_lane() {
        assert_eq!(appimage_arch_for("x86_64").expect("x86_64"), "x86_64");
        assert_eq!(appimage_arch_for("aarch64").expect("aarch64"), "aarch64");
        let err = appimage_arch_for("riscv64").expect_err("unmapped arch");
        assert!(err.to_string().contains("no AppImage architecture mapped"));
        assert_eq!(
            appimage_arch().expect("host arch"),
            appimage_arch_for(std::env::consts::ARCH).expect("host arch")
        );
    }

    #[test]
    fn appimagetool_retry_policy_retries_only_transient_statuses() {
        let retryable = [500, 502, 503, 504, 408, 429];
        let fatal = [200, 204, 301, 400, 401, 403, 404, 410];
        for code in retryable {
            let status = reqwest::StatusCode::from_u16(code).expect("parse retryable");
            assert!(is_retryable_status(status), "HTTP {code} must be retryable");
        }
        for code in fatal {
            let status = reqwest::StatusCode::from_u16(code).expect("parse fatal");
            assert!(
                !is_retryable_status(status),
                "HTTP {code} must fail fast, not retry"
            );
        }
    }

    #[test]
    fn appimage_runtime_pins_cover_both_ci_arches_on_one_frozen_tag() {
        assert_eq!(APPIMAGE_RUNTIMES.len(), 2, "exactly the two CI arches");
        for (arch, url, sha) in APPIMAGE_RUNTIMES {
            assert!(
                ["x86_64", "aarch64"].contains(arch),
                "unexpected arch row {arch}"
            );
            assert!(
                url.starts_with("https://github.com/AppImage/type2-runtime/releases/download/"),
                "runtime must come from the official type2-runtime releases"
            );
            assert!(
                !url.contains("/download/continuous/"),
                "the rolling `continuous` tag is not pinnable — freeze a release tag"
            );
            assert_eq!(sha.len(), 64, "sha256 pin must be a full 64-hex digest");
            assert!(
                sha.chars().all(|c| c.is_ascii_hexdigit()),
                "sha256 pin must be hex"
            );
            assert!(
                url.ends_with(&format!("/runtime-{arch}")),
                "URL asset name must match its arch row"
            );
        }
        let tags: std::collections::HashSet<&str> = APPIMAGE_RUNTIMES
            .iter()
            .map(|(_, url, _)| {
                url.split("/download/")
                    .nth(1)
                    .and_then(|rest| rest.split('/').next())
                    .expect("download path shape")
            })
            .collect();
        assert_eq!(tags.len(), 1, "all runtime rows share one frozen tag");
        assert!(
            appimage_runtime_for("x86_64").is_ok() && appimage_runtime_for("aarch64").is_ok(),
            "both CI arches resolve"
        );
        assert!(
            appimage_runtime_for("riscv64").is_err(),
            "unmapped arches fail instead of guessing"
        );
    }

    #[test]
    fn appimagetool_pins_cover_both_ci_arches_on_one_frozen_tag() {
        assert_eq!(APPIMAGETOOL_PINS.len(), 2, "exactly the two CI arches");
        for (arch, url, sha) in APPIMAGETOOL_PINS {
            assert!(
                ["x86_64", "aarch64"].contains(arch),
                "unexpected arch row {arch}"
            );
            assert!(
                url.starts_with("https://github.com/AppImage/appimagetool/releases/download/"),
                "tool must come from the official appimagetool releases"
            );
            assert!(
                !url.contains("/download/continuous/"),
                "the rolling `continuous` tag is not pinnable — freeze a release tag"
            );
            assert_eq!(sha.len(), 64, "sha256 pin must be a full 64-hex digest");
            assert!(
                sha.chars().all(|c| c.is_ascii_hexdigit()),
                "sha256 pin must be hex"
            );
            assert!(
                url.ends_with(&format!("/appimagetool-{arch}.AppImage")),
                "URL asset name must match its arch row"
            );
        }
        let tags: std::collections::HashSet<&str> = APPIMAGETOOL_PINS
            .iter()
            .map(|(_, url, _)| {
                url.split("/download/")
                    .nth(1)
                    .and_then(|rest| rest.split('/').next())
                    .expect("download path shape")
            })
            .collect();
        assert_eq!(tags.len(), 1, "all tool rows share one frozen tag");
        assert!(
            appimagetool_pin_for("x86_64").is_ok() && appimagetool_pin_for("aarch64").is_ok(),
            "both CI arches resolve a tool pin"
        );
        assert!(
            appimagetool_pin_for("riscv64").is_err(),
            "unmapped arches fail instead of guessing"
        );
    }

    #[test]
    fn appimagetool_retry_schedule_stays_inside_ci_job_budgets() {
        let waits = APPIMAGETOOL_RETRY_BACKOFF_SECS;
        assert_eq!(waits.len(), 4, "four backoff waits → five attempts");
        assert!(waits.iter().all(|w| *w > 0), "every wait is positive");
        assert!(
            waits.windows(2).all(|pair| pair[1] > pair[0]),
            "backoff strictly increases"
        );
        let total: u64 = waits.iter().sum();
        assert_eq!(total, 155, "155s total backoff");
        assert!(total < 60 * 10, "far under any job budget");
    }

    #[test]
    fn tool_cache_lands_atomically_at_the_final_path() {
        let root = temp_root("atomic-tool");
        let dest = root.join("tools").join("appimagetool-x86_64.AppImage");
        fs::create_dir_all(dest.parent().unwrap()).expect("tools dir");

        install_tool_bytes(&dest, b"first").expect("first install");
        assert_eq!(fs::read(&dest).expect("read dest"), b"first");
        assert!(
            !dest.with_extension("part").exists(),
            "no temp sibling left"
        );

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
