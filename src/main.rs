mod app;
mod cli;
mod handler;
mod input;
mod instance;
mod launch;
mod monitor;
mod paths;
mod util;

use crate::app::*;
use crate::cli::{parse_args, build_instances_from_cli, resolve_handler_from_cli, LaunchMode};
use crate::handler::Handler;
use crate::input::scan_input_devices;
use crate::instance::Instance;
use crate::monitor::*;
use crate::paths::PATH_PARTY;
use crate::util::*;

fn main() -> eframe::Result {
    // Our sdl/multimonitor stuff essentially depends on us running through x11.
    unsafe {
        std::env::set_var("SDL_VIDEODRIVER", "x11");
    }

    let monitors = get_monitors_sdl();

    println!("[partydeck] Monitors detected:");
    for monitor in &monitors {
        println!(
            "[partydeck] {} ({}x{})",
            monitor.name(),
            monitor.width(),
            monitor.height()
        );
    }

    let cli_args = parse_args();

    if cli_args.kwin {
        let args: Vec<String> = std::env::args().filter(|arg| arg != "--kwin").collect();

        let (w, h) = (monitors[0].width(), monitors[0].height());
        let mut cmd = std::process::Command::new("kwin_wayland");

        cmd.arg("--xwayland");
        cmd.arg("--width");
        cmd.arg(w.to_string());
        cmd.arg("--height");
        cmd.arg(h.to_string());
        cmd.arg("--exit-with-session");
        let args_string = args
            .iter()
            .map(|arg| format!("\"{}\"", arg))
            .collect::<Vec<String>>()
            .join(" ");
        cmd.arg(args_string);

        println!("[partydeck] Launching kwin session: {:?}", cmd);

        match cmd.spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("[partydeck] Failed to start kwin_wayland: {}", e);
                std::process::exit(1);
            }
        }
    }

    let mut handler_lite: Option<Handler> = None;
    let mut instances: Vec<Instance> = Vec::new();
    let auto_launch = cli_args.auto_launch;

    // Process CLI mode (handler or executable)
    if !matches!(cli_args.mode, LaunchMode::Gui) {
        handler_lite = match resolve_handler_from_cli(&cli_args.mode) {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("[partydeck] Error: {}", e);
                if matches!(cli_args.mode, LaunchMode::Handler(_)) {
                    cli::list_all_handlers();
                }
                std::process::exit(1);
            }
        };

        // Build instances if players were specified
        if !cli_args.players.is_empty() {
            let cfg = load_cfg();
            let input_devices = scan_input_devices(&cfg.pad_filter_type);
            let profiles = scan_profiles(true);
            
            match build_instances_from_cli(&cli_args.players, &input_devices, &profiles) {
                Ok(built_instances) => {
                    println!("[partydeck] Created {} instances from CLI", built_instances.len());
                    for (i, instance) in built_instances.iter().enumerate() {
                        println!("  Instance {}: {} devices, monitor {}", 
                            i + 1, 
                            instance.devices.len(),
                            instance.monitor
                        );
                    }
                    instances = built_instances;
                }
                Err(e) => {
                    eprintln!("[partydeck] Error building instances: {}", e);
                    cli::list_all_devices(&input_devices);
                    std::process::exit(1);
                }
            }
        } else if !auto_launch {
            // List devices if no players specified and not auto launching
            let cfg = load_cfg();
            let input_devices = scan_input_devices(&cfg.pad_filter_type);
            cli::list_all_devices(&input_devices);
        }
    }

    let fullscreen = cli_args.fullscreen;

    std::fs::create_dir_all(PATH_PARTY.join("gamesyms"))
        .expect("Failed to create gamesyms directory");
    std::fs::create_dir_all(PATH_PARTY.join("handlers"))
        .expect("Failed to create handlers directory");
    std::fs::create_dir_all(PATH_PARTY.join("profiles"))
        .expect("Failed to create profiles directory");

    remove_guest_profiles().unwrap();

    if PATH_PARTY.join("tmp").exists() {
        std::fs::remove_dir_all(PATH_PARTY.join("tmp")).unwrap();
    }

    let scrheight = monitors[0].height();

    let scale = match fullscreen {
        true => scrheight as f32 / 560.0,
        false => 1.3,
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 540.0])
            .with_min_inner_size([640.0, 360.0])
            .with_fullscreen(fullscreen)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../res/icon.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    println!("[partydeck] Starting eframe app...\n");

    eframe::run_native(
        "PartyDeck",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(scale);
            Ok(Box::<PartyApp>::new(PartyApp::new_with_cli(
                monitors.clone(),
                handler_lite,
                instances,
                auto_launch,
            )))
        }),
    )
}

