use std::fs;
use std::path::PathBuf;

use resvg::tiny_skia::Pixmap;

const ICON_SIZE: u32 = 256;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let svg_path = PathBuf::from("assets/tray/svg/mchose-logo.svg");
    println!("cargo:rerun-if-changed={}", svg_path.display());

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let png_path = out_dir.join("mchose-logo.png");
    let ico_path = out_dir.join("mchose-logo.ico");
    let svg = fs::read_to_string(&svg_path).expect("failed to read mchose-logo.svg");
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
        .expect("failed to parse mchose-logo.svg");
    let mut pixmap = Pixmap::new(ICON_SIZE, ICON_SIZE).expect("valid icon dimensions");
    let scale = ICON_SIZE as f32 / 26.0;
    let vertical_offset = (ICON_SIZE as f32 - 20.0 * scale) / 2.0;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_translate(0.0, vertical_offset).pre_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .save_png(&png_path)
        .expect("failed to render mchose-logo.png");
    write_ico(
        &ico_path,
        &fs::read(&png_path).expect("failed to read rendered logo"),
    );

    let mut resources = winresource::WindowsResource::new();
    resources.set_icon(ico_path.to_str().expect("icon path is valid UTF-8"));
    resources
        .compile()
        .expect("failed to embed application icon");
}

fn write_ico(path: &std::path::Path, png: &[u8]) {
    let mut ico = Vec::with_capacity(22 + png.len());
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&[0, 0, 0, 0]);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(png);
    fs::write(path, ico).expect("failed to write mchose-logo.ico");
}
