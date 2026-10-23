//! SPP_SMOKE harness (clean-audit item 71).
//!
//! The smoke used to return after the boot chain (migrations, boot
//! services, workers, loopback) and BEFORE the shell/renderer existed —
//! a renderer regression could not fail CI because the binary under
//! test never rendered a frame. The harness now constructs the REAL
//! [`ShellApp`] against a plain [`egui::Context`] (no window, no GPU —
//! the same tessellation path eframe drives) and renders one full pass
//! per page: `App::logic` + `App::ui`, every page key, and a
//! non-empty-paint assertion per pass. A panic or a blank frame fails
//! the smoke (and therefore CI).

use eframe::egui;

use crate::ShellOptions;
use crate::app::ShellApp;
use crate::pages::PageKey;

/// Construct the real shell headlessly and render one pass over every
/// page. Returns the number of painted shapes across all passes, or an
/// error naming the failing page.
///
/// # Errors
/// Fails when a page renders zero shapes (a blank/broken frame) — the
/// harness treats "painted nothing" as a renderer regression.
pub fn run_shell_smoke(options: ShellOptions) -> Result<usize, String> {
    let ctx = egui::Context::default();
    let mut app = ShellApp::new_with_context(&ctx, options);
    let mut total_shapes = 0usize;

    // Pass 1: the default page (Dashboard), exactly as a real boot
    // renders before any navigation.
    total_shapes += render_pass(&ctx, &mut app, PageKey::Dashboard)?;

    // One pass per page key: a regression in any page's renderer fails
    // the smoke instead of sailing through CI.
    for key in PageKey::all() {
        total_shapes += render_pass(&ctx, &mut app, key)?;
    }
    tracing::info!(
        operation = "smoke",
        pages = PageKey::all().len() + 1,
        shapes = total_shapes,
        "SPP_SMOKE: shell constructed headlessly, every page rendered"
    );
    Ok(total_shapes)
}

/// One headless frame: `App::logic` then `App::ui` on the page — the
/// same call order the eframe native runner uses — with a paint sanity
/// check.
fn render_pass(ctx: &egui::Context, app: &mut ShellApp, page: PageKey) -> Result<usize, String> {
    app.select_page(page);
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.logic_pass(ui.ctx());
        app.ui_pass(ui);
    });
    let shapes = output.shapes.len();
    output.drop_without_applying_deltas();
    if shapes == 0 {
        return Err(format!(
            "smoke: page {} rendered an empty frame (0 shapes)",
            page.label()
        ));
    }
    Ok(shapes)
}
