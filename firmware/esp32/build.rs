//! Gzips ../../web into the firmware image (see src/web.rs), so one flash
//! updates everything and the settings/recipes on LittleFS are never
//! overwritten. Also passes the ESP-IDF build environment through.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, fs};

use flate2::{write::GzEncoder, Compression, GzBuilder};

fn mime(path: &Path) -> Option<&'static str> {
    Some(match path.extension()?.to_str()? {
        "html" => "text/html",
        "js" => "text/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        _ => return None,
    })
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read web dir").flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk(&p, out);
        } else if mime(&p).is_some() {
            out.push(p);
        }
    }
}

fn gzip(data: &[u8]) -> Vec<u8> {
    // mtime 0 keeps the output reproducible.
    let mut enc: GzEncoder<Vec<u8>> = GzBuilder::new().mtime(0).write(Vec::new(), Compression::best());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

fn main() {
    embuild::espidf::sysenv::output();

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let web = manifest.join("../../web").canonicalize().expect("web/ next to firmware/");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", web.display());

    let mut files = Vec::new();
    walk(&web, &mut files);
    files.sort();

    let mut src = String::from("pub static WEB_ASSETS: &[WebAsset] = &[\n");
    for (i, path) in files.iter().enumerate() {
        println!("cargo:rerun-if-changed={}", path.display());
        let gz = out.join(format!("asset{i}.gz"));
        fs::write(&gz, gzip(&fs::read(path).unwrap())).unwrap();
        let url = format!("/{}", path.strip_prefix(&web).unwrap().to_string_lossy().replace('\\', "/"));
        src += &format!(
            "    WebAsset {{ path: {url:?}, mime: {:?}, data: include_bytes!({:?}) }},\n",
            mime(path).unwrap(),
            gz.display().to_string()
        );
    }
    src += "];\n";
    src += &format!(
        "pub static DEFAULT_RECIPES_JSON: &str = include_str!({:?});\n",
        web.join("default-recipes.json").display().to_string()
    );
    fs::write(out.join("web_assets.rs"), src).unwrap();
}
