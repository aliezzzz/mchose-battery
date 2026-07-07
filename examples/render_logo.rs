use std::fs;
use std::path::Path;

use resvg::tiny_skia::{Pixmap, Transform};

const SIZES: [u32; 4] = [16, 20, 24, 32];
const LOGO_ASPECT: f32 = 26.0 / 20.0;

fn main() -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let svg_path = root.join("assets/tray/svg/mchose-logo.svg");
    let png_dir = root.join("assets/tray/png");

    let svg = fs::read_to_string(svg_path)?;
    render_variant(&svg, &png_dir, "light", "black")?;
    render_variant(&svg, &png_dir, "dark", "#f4f4f4")?;

    Ok(())
}

fn render_variant(svg: &str, png_dir: &Path, suffix: &str, fill: &str) -> anyhow::Result<()> {
    let svg = svg.replace("fill=\"black\"", &format!("fill=\"{fill}\""));
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &options)?;

    for size in SIZES {
        let mut pixmap = Pixmap::new(size, size).expect("valid pixmap size");
        let content_width = size as f32;
        let content_height = (content_width / LOGO_ASPECT).round();
        let scale = content_width / 26.0;
        let tx = (size as f32 - content_width) / 2.0;
        let ty = (size as f32 - content_height) / 2.0;
        let transform = Transform::from_translate(tx, ty).pre_scale(scale, scale);

        resvg::render(&tree, transform, &mut pixmap.as_mut());
        pixmap.save_png(png_dir.join(format!("mchose-logo-{suffix}-{size}.png")))?;
    }

    Ok(())
}
