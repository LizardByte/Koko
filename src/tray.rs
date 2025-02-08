#![doc = "Tray icon utilities for the application."]

// lib imports
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    TrayIconBuilder, TrayIconEvent,
};

// local imports
use crate::globals;

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
}

/// Launch the tray icon and event loop.
pub fn launch() {
    let path = std::path::Path::new(globals::GLOBAL_ICON_ICO_PATH);

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // set a tray event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        proxy
            .send_event(UserEvent::TrayIconEvent(event))
            .expect("Tray icon event handler failed");
    }));

    // set a menu event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        proxy
            .send_event(UserEvent::MenuEvent(event))
            .expect("Menu event handler failed");
    }));

    let tray_menu = Menu::new();

    // top level items
    let open_i = MenuItem::new(format!("Open {}", globals::GLOBAL_APP_NAME), true, None);
    let about_i = PredefinedMenuItem::about(
        None,
        Some(AboutMetadata {
            name: Some(globals::GLOBAL_APP_NAME.to_string()),
            copyright: Some("© LizardByte".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..Default::default()
        }),
    );
    let quit_i = MenuItem::new("Quit", true, None);

    // submenus
    let donate_menu = Submenu::new("Donate", true);
    let options_menu = Submenu::new("Quick Options", true);
    let api_menu = Submenu::new("API Docs", true);

    // donate submenu items
    let donate_github_i = MenuItem::new("GitHub", true, None);
    let donate_patreon_i = MenuItem::new("Patreon", true, None);
    let donate_paypal_i = MenuItem::new("PayPal", true, None);
    donate_menu
        .append_items(&[
            &donate_github_i,
            &donate_patreon_i,
            &donate_paypal_i,
        ])
        .expect("Failed to append items to menu");

    // options submenu items
    let options_disable_tray_i = MenuItem::new("Disable Tray Icon", true, None);
    let options_settings_i = MenuItem::new("Settings", true, None);
    options_menu
        .append_items(&[
            &options_disable_tray_i,
            &options_settings_i,
        ])
        .expect("Failed to append items to menu");

    // api submenu items
    let api_rapidoc_i = MenuItem::new("RapiDoc", true, None);
    let api_swagger_i = MenuItem::new("Swagger", true, None);
    api_menu
        .append_items(&[&api_rapidoc_i, &api_swagger_i])
        .expect("Failed to append items to menu");

    tray_menu
        .append_items(&[
            &open_i,
            &PredefinedMenuItem::separator(),
            &donate_menu,
            &options_menu,
            &api_menu,
            &PredefinedMenuItem::separator(),
            &about_i,
            &quit_i,
        ])
        .expect("Failed to append items to menu");

    let mut tray_icon = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                let icon = load_icon(std::path::Path::new(path));

                // We create the icon once the event loop is actually running
                // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_icon(icon)
                        .with_menu(Box::new(tray_menu.clone()))
                        .with_tooltip(globals::GLOBAL_APP_NAME)
                        .build()
                        .unwrap(),
                );

                // We have to request a redraw here to have the icon actually show up.
                // Tao only exposes a redraw method on the Window so we use core-foundation directly.
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc2_core_foundation::{CFRunLoopGetMain, CFRunLoopWakeUp};

                    let rl = CFRunLoopGetMain().unwrap();
                    CFRunLoopWakeUp(&rl);
                }
            }

            Event::UserEvent(UserEvent::TrayIconEvent(event)) => {
                log::debug!("Tray icon event: {:?}", event);
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                log::debug!("Tray Menu event: {:?}", event);

                match event.id {
                    id if id == quit_i.id() => {
                        tray_icon.take();
                        *control_flow = ControlFlow::Exit;
                    }
                    id if id == options_disable_tray_i.id() => {
                        // TODO: adjust application config first
                        tray_icon.as_mut().unwrap().set_visible(false).unwrap();
                    }
                    id if id == open_i.id() => {
                        webbrowser::open(globals::get_server_url().as_str()).unwrap()
                    }
                    id if id == donate_github_i.id() => {
                        webbrowser::open("https://github.com/sponsors/LizardByte").unwrap()
                    }
                    id if id == donate_patreon_i.id() => {
                        webbrowser::open("https://patreon.com/LizardByte").unwrap()
                    }
                    id if id == donate_paypal_i.id() => {
                        webbrowser::open("https://www.paypal.com/paypalme/ReenigneArcher").unwrap()
                    }
                    id if id == options_settings_i.id() => {
                        webbrowser::open(&format!("{}/settings", globals::get_server_url()))
                            .unwrap()
                    }
                    id if id == api_rapidoc_i.id() => {
                        webbrowser::open(&format!("{}/rapidoc", globals::get_server_url())).unwrap()
                    }
                    id if id == api_swagger_i.id() => {
                        webbrowser::open(&format!("{}/swagger-ui", globals::get_server_url()))
                            .unwrap()
                    }
                    _ => {
                        log::error!("Unknown menu event: {:?}", event);
                    }
                }
            }

            _ => {}
        }
    })
}

/// Load an icon from a file path.
pub fn load_icon(path: &std::path::Path) -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
