//! Builds the web app (npm, in ../../web) and gzips its dist/ folder into the
//! firmware image (see src/web.rs), so one flash updates everything and the
//! settings/recipes on LittleFS are never overwritten. Also passes the ESP-IDF
//! build environment through.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

/// Inputs to the web build. dist/ and node_modules/ are outputs, so they are
/// not watched (watching them would rebuild on every cargo run).
const WEB_INPUTS: &[&str] = &[
    "src",
    "build.mjs",
    "package.json",
    "package-lock.json",
    "tsconfig.json",
    "index.html",
    "style.css",
    "icon.svg",
    "manifest.webmanifest",
    "default-recipes.json",
];

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

fn modified(p: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn npm(web: &Path, args: &[&str]) {
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let status = Command::new(npm).args(args).current_dir(web).status().unwrap_or_else(|e| {
        panic!("could not run `npm` ({e}). The web app needs Node.js 18 or newer: https://nodejs.org")
    });
    if !status.success() {
        panic!("`npm {}` failed in {}", args.join(" "), web.display());
    }
}

fn build_web(web: &Path) {
    // Reinstall when package-lock.json is newer than the last install.
    let installed = modified(&web.join("node_modules/.package-lock.json"));
    if installed.is_none() || installed < modified(&web.join("package-lock.json")) {
        npm(web, &["ci"]);
    }
    npm(web, &["run", "build"]);
}

fn main() {
    embuild::espidf::sysenv::output();

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let web = manifest.join("../../web").canonicalize().expect("web/ next to firmware/");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    for input in WEB_INPUTS {
        println!("cargo:rerun-if-changed={}", web.join(input).display());
    }
    build_web(&web);
    let dist = web.join("dist");

    let mut files = Vec::new();
    walk(&dist, &mut files);
    files.sort();

    let mut src = String::from("pub static WEB_ASSETS: &[WebAsset] = &[\n");
    for (i, path) in files.iter().enumerate() {
        let gz = out.join(format!("asset{i}.gz"));
        fs::write(&gz, gzip(&fs::read(path).unwrap())).unwrap();
        let url = format!("/{}", path.strip_prefix(&dist).unwrap().to_string_lossy().replace('\\', "/"));
        src += &format!(
            "    WebAsset {{ path: {url:?}, mime: {:?}, data: include_bytes!({:?}) }},\n",
            mime(path).unwrap(),
            gz.display().to_string()
        );
    }
    src += "];\n";
    src += &format!(
        "#[allow(dead_code)] // Wokwi builds install their own test recipes\npub static DEFAULT_RECIPES_JSON: &str = include_str!({:?});\n",
        web.join("default-recipes.json").display().to_string()
    );
    fs::write(out.join("web_assets.rs"), src).unwrap();
}
