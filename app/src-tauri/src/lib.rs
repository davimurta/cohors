pub fn run() -> tauri::Result<()> {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init());
    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(mac_menu::build)
        .on_menu_event(mac_menu::handle);
    builder.run(tauri::generate_context!())
}

/// The macOS menu bar. The default one closes the window with Cmd+W, which
/// Cohors uses to close a split pane; closing the window moves to
/// Shift+Cmd+W. Other platforms have no menu bar and handle the shortcut
/// in the page.
#[cfg(target_os = "macos")]
mod mac_menu {
    use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
    use tauri::{AppHandle, Emitter, Manager, Runtime};

    const CLOSE_PANE: &str = "close-pane";
    const CLOSE_WINDOW: &str = "close-window";

    pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
        let name = app.package_info().name.clone();
        let app_menu = Submenu::with_items(
            app,
            &name,
            true,
            &[
                &PredefinedMenuItem::about(app, None, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::services(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::hide_others(app, None)?,
                &PredefinedMenuItem::show_all(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::quit(app, None)?,
            ],
        )?;
        // Without these items the page loses copy and paste on macOS.
        let edit_menu = Submenu::with_items(
            app,
            "Editar",
            true,
            &[
                &PredefinedMenuItem::undo(app, None)?,
                &PredefinedMenuItem::redo(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::cut(app, None)?,
                &PredefinedMenuItem::copy(app, None)?,
                &PredefinedMenuItem::paste(app, None)?,
                &PredefinedMenuItem::select_all(app, None)?,
            ],
        )?;
        let window_menu = Submenu::with_items(
            app,
            "Janela",
            true,
            &[
                &MenuItem::with_id(app, CLOSE_PANE, "Fechar painel", true, Some("Cmd+W"))?,
                &MenuItem::with_id(
                    app,
                    CLOSE_WINDOW,
                    "Fechar janela",
                    true,
                    Some("Shift+Cmd+W"),
                )?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::minimize(app, None)?,
                &PredefinedMenuItem::maximize(app, None)?,
                &PredefinedMenuItem::fullscreen(app, None)?,
            ],
        )?;
        Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])
    }

    pub fn handle<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
        match event.id().as_ref() {
            CLOSE_PANE => {
                if let Err(error) = app.emit(CLOSE_PANE, ()) {
                    eprintln!("failed to send {CLOSE_PANE}: {error}");
                }
            }
            CLOSE_WINDOW => {
                if let Some(window) = app.get_webview_window("main")
                    && let Err(error) = window.close()
                {
                    eprintln!("failed to close the window: {error}");
                }
            }
            _ => {}
        }
    }
}
