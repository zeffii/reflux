//! Reflux: a demo tool driven by Wax animation lanes.
//!
//! `reflux`                  opens the window
//! `reflux --shot out.png`   renders one frame headless (see `--help`)

mod app;
mod events;
mod camera;
mod geo;
mod gpu;
mod graph;
mod mesh;
mod overlay;
mod project;
mod render;
mod scene;
mod shot;
mod view;

use view::{DimSetting, Overlay};

const HELP: &str = "\
reflux [--scene FILE.reflux] [--obj FILE.obj]
                            open the editor window (you can also drop an .obj on it);
                            the scene defaults to scenes/demo.reflux when it exists
reflux --shot FILE.png      render one frame without a window
    --time SECONDS          song position (default 4.0)
    --size WxH              image size (default 1280x720)
    --overlay graph|lane    overlay to draw (default none)
    --dim 0-3               dim level for that overlay
    --soft / --no-soft      blur and desaturate the backdrop
    --no-status             leave out the status line
    --obj FILE.obj          load a mesh (replaces the SDF core)
    --keep-core             show the SDF core as well as the mesh
    --scene FILE.reflux     the graph to evaluate";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return;
    }
    match parse_shot(&args) {
        Ok(Some(options)) => shot::run(options),
        Ok(None) => app::run(path_argument(&args, "--obj"), scene_argument(&args)),
        Err(message) => {
            eprintln!("{message}\n\n{HELP}");
            std::process::exit(2);
        }
    }
}

fn parse_shot(args: &[String]) -> Result<Option<shot::ShotOptions>, String> {
    let mut options = shot::ShotOptions {
        path: String::new(),
        seconds: 4.0,
        size: [1280, 720],
        overlay: None,
        dim: None,
        show_status: true,
        obj_path: None,
        scene_path: None,
        keep_core: false,
    };
    let mut dim_level = None;
    let mut soft = None;
    let mut is_shot = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--shot" => {
                is_shot = true;
                options.path = value()?;
            }
            "--time" => options.seconds = value()?.parse().map_err(|_| "bad --time".to_string())?,
            "--size" => options.size = parse_size(&value()?)?,
            "--overlay" => {
                options.overlay = match value()?.as_str() {
                    "graph" => Some(Overlay::Graph),
                    "lane" => Some(Overlay::Lane),
                    "none" => None,
                    other => return Err(format!("unknown overlay {other}")),
                }
            }
            "--dim" => dim_level = Some(value()?.parse::<usize>().map_err(|_| "bad --dim".to_string())?),
            "--soft" => soft = Some(true),
            "--no-soft" => soft = Some(false),
            "--no-status" => options.show_status = false,
            "--obj" => options.obj_path = Some(value()?.into()),
            "--keep-core" => options.keep_core = true,
            "--scene" => options.scene_path = Some(value()?.into()),
            other => return Err(format!("unknown argument {other}")),
        }
    }

    if !is_shot {
        return Ok(None);
    }
    if options.scene_path.is_none() {
        options.scene_path = scene_argument(args);
    }
    if dim_level.is_some() || soft.is_some() {
        // Start from the overlay's own default and override what was given.
        let mut view = view::ViewState::default();
        view.overlay = options.overlay;
        let default = view.dim_setting();
        options.dim = Some(DimSetting {
            level: dim_level.unwrap_or(default.level),
            soft: soft.unwrap_or(default.soft),
        });
    }
    Ok(Some(options))
}

fn path_argument(args: &[String], flag: &str) -> Option<std::path::PathBuf> {
    let index = args.iter().position(|a| a == flag)?;
    args.get(index + 1).map(|path| path.into())
}

/// `--scene FILE`, or the demo scene when it exists next to where Reflux was started.
fn scene_argument(args: &[String]) -> Option<std::path::PathBuf> {
    path_argument(args, "--scene").or_else(|| {
        let default = std::path::PathBuf::from(project::DEFAULT_SCENE);
        default.exists().then_some(default)
    })
}

fn parse_size(text: &str) -> Result<[u32; 2], String> {
    let (w, h) = text.split_once('x').ok_or("size must look like 1280x720")?;
    let w = w.parse().map_err(|_| "bad width")?;
    let h = h.parse().map_err(|_| "bad height")?;
    Ok([w, h])
}
