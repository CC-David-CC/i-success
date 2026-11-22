//! Runs the upstream views and practice engine in a browser canvas.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg(target_arch = "wasm32")]

use i_success_ui_egui::App;
use wasm_bindgen::prelude::*;

struct Shell {
    app: App,
    context: egui::Context,
}

impl eframe::App for Shell {
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.show(ui);
    }
}

/// One independent practice view. Its host chooses where the document is stored.
#[wasm_bindgen]
pub struct PracticeWindow {
    runner: eframe::WebRunner,
}

impl Default for PracticeWindow {
    fn default() -> Self {
        Self { runner: eframe::WebRunner::new() }
    }
}

#[wasm_bindgen]
impl PracticeWindow {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|creation| Ok(Box::new(Shell { app: App::new(), context: creation.egui_ctx.clone() }))),
            )
            .await
    }
    pub fn document(&self) -> Result<String, JsValue> {
        let shell = self.runner.app_mut::<Shell>().ok_or_else(|| JsValue::from_str("The practice window is closed."))?;
        shell.app.document().map_err(|e| JsValue::from_str(&e))
    }
    pub fn load(&self, content: &str) -> Result<(), JsValue> {
        let mut shell = self.runner.app_mut::<Shell>().ok_or_else(|| JsValue::from_str("The practice window is closed."))?;
        shell.app.load_document(content).map_err(|e| JsValue::from_str(&e))?;
        shell.context.request_repaint();
        Ok(())
    }
    pub fn close(&self) {
        self.runner.destroy();
    }
}
