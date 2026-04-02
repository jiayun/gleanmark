use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show GleanMark", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn register_global_shortcut(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let shortcut: Shortcut = "CmdOrCtrl+Shift+G".parse()?;

    app.global_shortcut().on_shortcut(shortcut, |app, _scut, event| {
        if event.state == ShortcutState::Pressed {
            show_main_window(app);
        }
    })?;

    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

pub fn check_for_updates(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let updater = match handle.updater() {
            Ok(u) => u,
            Err(e) => {
                eprintln!("Updater not available: {e}");
                show_update_result(&handle, &format!("Updater error: {e}"));
                return;
            }
        };

        match updater.check().await {
            Ok(Some(update)) => {
                let version = update.version.clone();
                show_update_result(
                    &handle,
                    &format!("Version {} available! Downloading...", version),
                );
                let mut downloaded = 0;
                if let Err(e) = update
                    .download_and_install(
                        |chunk, _total| {
                            downloaded += chunk;
                            eprintln!("Downloaded {} bytes", downloaded);
                        },
                        || {
                            eprintln!("Download complete, installing...");
                        },
                    )
                    .await
                {
                    show_update_result(&handle, &format!("Update failed: {e}"));
                } else {
                    show_update_result(&handle, "Update installed! Restart to apply.");
                }
            }
            Ok(None) => {
                show_update_result(&handle, "You're running the latest version.");
            }
            Err(e) => {
                show_update_result(&handle, &format!("Check failed: {e}"));
            }
        }
    });
}

fn show_update_result(handle: &AppHandle, message: &str) {
    if let Some(win) = handle.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        let js = format!("alert('{}')", message.replace('\'', "\\'"));
        let _ = win.eval(&js);
    }
}
