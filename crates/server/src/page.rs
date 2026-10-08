//! The built web page, which the program carries inside it.

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

mod built {
    include!(concat!(env!("OUT_DIR"), "/web.rs"));
}

/// Whether the program was built with the web page in it.
pub fn is_built() -> bool {
    !built::FILES.is_empty()
}

/// The file of a path, or the page itself for a route that the page finds in the address.
pub async fn serve(uri: Uri) -> Response {
    answer(built::FILES, uri.path())
}

fn answer(files: &'static [(&'static str, &'static [u8])], path: &str) -> Response {
    let route = path.trim_start_matches('/');
    let route = if route.is_empty() {
        "index.html"
    } else {
        route
    };
    let found = files.iter().find(|(name, _)| *name == route);
    let (name, bytes) = match found {
        Some(file) => *file,
        // A route has no extension. A missing script or picture is missing, not the page.
        None if !route.rsplit('/').next().unwrap_or("").contains('.') => {
            match files.iter().find(|(name, _)| *name == "index.html") {
                Some(file) => *file,
                None => return StatusCode::NOT_FOUND.into_response(),
            }
        }
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    // Files under assets have a hash in their name, so they never change.
    let cache = if name.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, mime(name)),
            (header::CACHE_CONTROL, cache),
        ],
        bytes,
    )
        .into_response()
}

fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[(&str, &[u8])] = &[
        ("index.html", b"<title>Stubprint</title>"),
        ("assets/app-1a2b.js", b"console.log(1)"),
    ];

    fn status(path: &str) -> u16 {
        answer(FILES, path).status().as_u16()
    }

    #[test]
    fn a_file_is_served_with_its_type_and_a_route_of_the_page_is_the_page() {
        let script = answer(FILES, "/assets/app-1a2b.js");
        assert_eq!(script.status(), 200);
        assert_eq!(
            script.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert!(script.headers()[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable"));
        assert_eq!(status("/"), 200);
        assert_eq!(status("/receipts/3"), 200);
        assert_eq!(
            answer(FILES, "/printers").headers()[header::CACHE_CONTROL],
            "no-cache"
        );
    }

    #[test]
    fn a_missing_file_is_missing_and_without_a_page_nothing_is_served() {
        assert_eq!(status("/assets/gone.js"), 404);
        assert_eq!(answer(&[], "/").status(), 404);
        assert_eq!(answer(&[], "/printers").status(), 404);
    }
}
