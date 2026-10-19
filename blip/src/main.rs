#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "blip",
        native_options,
        Box::new(|cc| Ok(Box::new(blip::App::new(cc)))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    use eframe::wasm_bindgen::JsCast as _;

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("No window");
        let document = window.document().expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let runner = eframe::WebRunner::new();
        let start_result = runner
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(blip::App::new(cc)))),
            )
            .await;
        if start_result.is_ok()
            && let Err(e) = save_when_hidden(&runner, &window, &document)
        {
            log::warn!("Failed to save the app state when the page is left: {e:?}");
        }

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

/// Saves the app state whenever the page is hidden or left, e.g. on a reload or when the
/// tab is closed. eframe saves it on its own only every 30 seconds: its save on closing
/// listens to `onbeforeunload`, which is not the name of an event, so it never runs.
#[cfg(target_arch = "wasm32")]
fn save_when_hidden(
    runner: &eframe::WebRunner,
    window: &web_sys::Window,
    document: &web_sys::Document,
) -> Result<(), eframe::wasm_bindgen::JsValue> {
    runner.add_event_listener(window, "pagehide", |_: web_sys::Event, app| app.save())?;
    let hidden = document.clone();
    runner.add_event_listener(
        document,
        "visibilitychange",
        move |_: web_sys::Event, app| {
            if hidden.hidden() {
                app.save();
            }
        },
    )
}
