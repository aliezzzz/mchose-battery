use std::fs;
use std::path::Path;

use resvg::tiny_skia::{Pixmap, Transform};

const TRAY_SIZE: u32 = 32;
const MENU_SIZE: u32 = 16;

const DARK_BG_FILL: &str = "#ffffff";
const LIGHT_BG_FILL: &str = "#000000";

struct Variant {
    name: &'static str,
    source: &'static str,
}

const BATTERY_VARIANTS: &[Variant] = &[
    Variant {
        name: "0",
        source: "battery-line",
    },
    Variant {
        name: "1",
        source: "battery-low-line",
    },
    Variant {
        name: "2",
        source: "battery-medium",
    },
    Variant {
        name: "3",
        source: "battery-max",
    },
];

struct StatusVariant {
    name: &'static str,
    fill: &'static str,
}

const STATUS_VARIANTS: &[StatusVariant] = &[
    StatusVariant {
        name: "normal",
        fill: "#09d36b",
    },
    StatusVariant {
        name: "medium",
        fill: "#f5a623",
    },
    StatusVariant {
        name: "low",
        fill: "#ff4d4f",
    },
    StatusVariant {
        name: "unknown",
        fill: "#9ca3af",
    },
];

fn main() -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let svg_dir = root.join("assets/tray/svg/newicon");
    let png_dir = root.join("assets/tray/png");

    fs::create_dir_all(&png_dir)?;

    render_unknown(&svg_dir, &png_dir)?;

    for variant in BATTERY_VARIANTS {
        render_battery_pair(&svg_dir, &png_dir, variant, false)?;
    }

    render_charging_pair(&svg_dir, &png_dir)?;

    for variant in STATUS_VARIANTS {
        render_status(&svg_dir, &png_dir, variant)?;
    }

    Ok(())
}

fn render_unknown(svg_dir: &Path, png_dir: &Path) -> anyhow::Result<()> {
    let svg = load_svg(svg_dir, "battery-line")?;
    render_battery_tight(
        &svg,
        DARK_BG_FILL,
        TRAY_SIZE,
        png_dir.join("battery-unknown-dark.png"),
    )?;
    render_battery_tight(
        &svg,
        LIGHT_BG_FILL,
        TRAY_SIZE,
        png_dir.join("battery-unknown-light.png"),
    )?;
    Ok(())
}

fn render_battery_pair(
    svg_dir: &Path,
    png_dir: &Path,
    variant: &Variant,
    charging: bool,
) -> anyhow::Result<()> {
    let svg = load_svg(svg_dir, variant.source)?;
    let prefix = if charging {
        "battery-charging"
    } else {
        "battery"
    };
    render_battery_tight(
        &svg,
        DARK_BG_FILL,
        TRAY_SIZE,
        png_dir.join(format!("{prefix}-{name}-dark.png", name = variant.name)),
    )?;
    render_battery_tight(
        &svg,
        LIGHT_BG_FILL,
        TRAY_SIZE,
        png_dir.join(format!("{prefix}-{name}-light.png", name = variant.name)),
    )?;
    Ok(())
}

fn render_charging_pair(svg_dir: &Path, png_dir: &Path) -> anyhow::Result<()> {
    let svg = load_svg(svg_dir, "battery-charge-line")?;
    render_battery_tight(
        &svg,
        DARK_BG_FILL,
        TRAY_SIZE,
        png_dir.join("battery-charging-dark.png"),
    )?;
    render_battery_tight(
        &svg,
        LIGHT_BG_FILL,
        TRAY_SIZE,
        png_dir.join("battery-charging-light.png"),
    )?;
    Ok(())
}

fn render_status(svg_dir: &Path, png_dir: &Path, variant: &StatusVariant) -> anyhow::Result<()> {
    let svg = load_svg(svg_dir, "status")?;
    render_to_png(
        &svg,
        variant.fill,
        MENU_SIZE,
        png_dir.join(format!("battery-status-{}.png", variant.name)),
    )?;
    Ok(())
}

fn load_svg(svg_dir: &Path, name: &str) -> anyhow::Result<String> {
    let path = svg_dir.join(format!("{name}.svg"));
    fs::read_to_string(&path).map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))
}

fn render_to_png(svg: &str, fill: &str, size: u32, out: impl AsRef<Path>) -> anyhow::Result<()> {
    render_battery_tight(svg, fill, size, out)
}

fn render_battery_tight(
    svg: &str,
    fill: &str,
    size: u32,
    out: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let svg = svg.replace("fill=\"#000000\"", &format!("fill=\"{fill}\""));
    let svg = svg.replace("fill=\"#09d36b\"", &format!("fill=\"{fill}\""));

    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &options)?;

    let bbox = tree.root().abs_bounding_box();
    if bbox.width() <= 0.0 || bbox.height() <= 0.0 {
        anyhow::bail!("SVG has no visible content");
    }

    let padding = 2.0;
    let target = size as f32 - padding * 2.0;
    let scale = (target / bbox.width()).min(target / bbox.height());

    let offset_x = (size as f32 - bbox.width() * scale) / 2.0 - bbox.x() * scale;
    let offset_y = (size as f32 - bbox.height() * scale) / 2.0 - bbox.y() * scale;

    let mut pixmap = Pixmap::new(size, size).expect("valid pixmap size");
    let transform = Transform::from_translate(offset_x, offset_y).pre_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap.save_png(out)?;
    Ok(())
}
