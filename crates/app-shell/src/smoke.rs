use eframe::egui;

use crate::ShellOptions;
use crate::app::ShellApp;
use crate::pages::PageKey;

pub fn run_shell_smoke(options: ShellOptions) -> Result<usize, String> {
    let ctx = egui::Context::default();
    let mut app = ShellApp::new_with_context(&ctx, options);
    let mut total_shapes = 0usize;

    total_shapes += render_pass(&ctx, &mut app, PageKey::Dashboard)?;

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
