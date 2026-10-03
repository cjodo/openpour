//! The web app, gzipped into the image by build.rs.

pub struct WebAsset {
    pub path: &'static str,
    pub mime: &'static str,
    /// Gzip-compressed contents.
    pub data: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

pub fn find(path: &str) -> Option<&'static WebAsset> {
    WEB_ASSETS.iter().find(|a| a.path == path)
}
