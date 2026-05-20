// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod state;
mod server;
mod cli;
mod orchestrator;
mod memory;
mod config;

use std::sync::Arc;
use std::sync::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent,
};

struct ServerPort(u16);
struct Quitting(Mutex<bool>);
struct AppConfigState(Arc<Mutex<config::AppConfig>>); // None = unset, Some(true) = hide to tray, Some(false) = quit

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let port: u16 = 5556;
            app.manage(ServerPort(port));
            app.manage(Quitting(Mutex::new(false)));

            // Load config — shared between Tauri and Axum server
            let app_config = Arc::new(Mutex::new(config::load()));
            app.manage(AppConfigState(app_config.clone()));
            let config_for_server = app_config;

            // Initialize shared app state (database, etc.)
            let app_state = Arc::new(
                state::State::init().expect("Failed to initialize app state"),
            );
            app.manage(app_state.clone());

            // Spawn memory tracker: update window title every 3s
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let mb = memory::memory_usage_mb();
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let title = format!("Backyard — {:.0} MB", mb);
                        let _ = window.set_title(&title);
                    }
                }
            });

            // Spawn the Axum HTTP server in a background task
            let state_clone = app_state.clone();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = server::start_server(state_clone, config_for_server).await {
                    eprintln!("Server error: {}", e);
                    app_handle.exit(1);
                }
            });

            // Build tray menu
            let show = MenuItemBuilder::with_id("show", "Show Backyard").build(app)?;
            let hide = MenuItemBuilder::with_id("hide", "Hide to Tray").build(app)?;
            let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show)
                .item(&hide)
                .item(&separator)
                .item(&quit)
                .build()?;

            // Build tray icon
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Backyard - LLM Lab")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "quit" => {
                        if let Some(window) = app.get_webview_window("main") {
                            *app.state::<Quitting>().0.lock().unwrap() = true;
                            let _ = window.close();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();

                // Programmatic close — let through
                if *app.state::<Quitting>().0.lock().unwrap() {
                    return;
                }

                let close_to_tray = app
                    .state::<AppConfigState>()
                    .0
                    .lock()
                    .unwrap()
                    .close_to_tray;

                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // else: don't prevent close — window closes, Tauri exits gracefully
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                RunEvent::Ready => {
                    let port = app_handle.state::<ServerPort>().0;
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let url = format!("http://localhost:{}", port);
                        let _ = window.navigate(url.parse().unwrap());
                    }
                }
                _ => {}
            }
        });
}
