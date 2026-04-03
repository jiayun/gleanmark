use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
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
                show_error(&handle, &format!("Updater error: {e}"));
                return;
            }
        };

        let update = match updater.check().await {
            Ok(Some(update)) => update,
            Ok(None) => {
                handle
                    .dialog()
                    .message("You're running the latest version.")
                    .title("GleanMark")
                    .kind(MessageDialogKind::Info)
                    .blocking_show();
                return;
            }
            Err(e) => {
                show_error(&handle, &format!("Check failed: {e}"));
                return;
            }
        };

        // Ask user to confirm update
        let version = update.version.clone();
        let confirmed = handle
            .dialog()
            .message(format!("Version {version} is available. Download and install?"))
            .title("GleanMark Update")
            .kind(MessageDialogKind::Info)
            .buttons(MessageDialogButtons::OkCancelCustom("Update".into(), "Later".into()))
            .blocking_show();

        if !confirmed {
            return;
        }

        // Show progress overlay in webview
        show_progress(&handle, 0, "Starting download...");

        let mut downloaded: u64 = 0;
        let handle_for_progress = handle.clone();

        let result = update
            .download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length as u64;
                    let percent = content_length
                        .map(|total| ((downloaded as f64 / total as f64) * 100.0) as u32)
                        .unwrap_or(0);
                    let size_text = format_size(downloaded, content_length);
                    show_progress(&handle_for_progress, percent, &size_text);
                },
                || {
                    show_progress(&handle_for_progress, 100, "Installing...");
                },
            )
            .await;

        // Remove progress overlay
        hide_progress(&handle);

        match result {
            Ok(()) => {
                let restart = handle
                    .dialog()
                    .message("Update installed successfully. Restart now?")
                    .title("GleanMark Update")
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "Restart".into(),
                        "Later".into(),
                    ))
                    .blocking_show();

                if restart {
                    handle.restart();
                }
            }
            Err(e) => {
                show_error(&handle, &format!("Update failed: {e}"));
            }
        }
    });
}

fn show_error(handle: &AppHandle, message: &str) {
    handle
        .dialog()
        .message(message)
        .title("GleanMark")
        .kind(MessageDialogKind::Error)
        .blocking_show();
}

fn show_progress(handle: &AppHandle, percent: u32, text: &str) {
    let Some(win) = handle.get_webview_window("main") else { return };
    let _ = win.show();

    let js = format!(
        r#"(function() {{
  let overlay = document.getElementById('gm-update-overlay');
  if (!overlay) {{
    overlay = document.createElement('div');
    overlay.id = 'gm-update-overlay';
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:99999;background:var(--bg-secondary,#f0f0f0);padding:12px 20px;display:flex;align-items:center;gap:12px;font-family:system-ui;font-size:14px;box-shadow:0 2px 8px rgba(0,0,0,0.15)';
    const label = document.createElement('span');
    label.id = 'gm-update-label';
    label.style.cssText = 'white-space:nowrap';
    const barBg = document.createElement('div');
    barBg.style.cssText = 'flex:1;height:6px;background:var(--border-color,#ddd);border-radius:3px;overflow:hidden';
    const bar = document.createElement('div');
    bar.id = 'gm-update-bar';
    bar.style.cssText = 'height:100%;background:#3b82f6;border-radius:3px;transition:width 0.3s ease';
    barBg.appendChild(bar);
    const pct = document.createElement('span');
    pct.id = 'gm-update-pct';
    pct.style.cssText = 'min-width:40px;text-align:right;font-variant-numeric:tabular-nums';
    overlay.appendChild(label);
    overlay.appendChild(barBg);
    overlay.appendChild(pct);
    document.body.prepend(overlay);
  }}
  document.getElementById('gm-update-bar').style.width = '{percent}%';
  document.getElementById('gm-update-pct').textContent = '{percent}%';
  document.getElementById('gm-update-label').textContent = '{text}';
}})();"#,
        percent = percent,
        text = text.replace('\'', "\\'"),
    );
    let _ = win.eval(&js);
}

fn hide_progress(handle: &AppHandle) {
    let Some(win) = handle.get_webview_window("main") else { return };
    let js = r#"(function() {
  const el = document.getElementById('gm-update-overlay');
  if (el) el.remove();
})();"#;
    let _ = win.eval(js);
}

fn format_size(downloaded: u64, total: Option<u64>) -> String {
    let dl = format_bytes(downloaded);
    match total {
        Some(t) => format!("{dl} / {}", format_bytes(t)),
        None => dl,
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
