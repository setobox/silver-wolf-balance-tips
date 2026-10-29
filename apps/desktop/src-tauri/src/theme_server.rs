//! Theme documents use an actual, unprivileged origin. Registered Tauri custom
//! protocols are classified as local by Tauri ACL and must not host scripts.
use crate::{assets, Shared};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::Response,
    routing::get,
    Router,
};
use std::sync::Arc;

pub fn start(listener: std::net::TcpListener, shared: Arc<Shared>) -> Result<(), std::io::Error> {
    listener.set_nonblocking(true)?;
    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(_) => return,
        };
        let app = Router::new().fallback(get(resource)).with_state(shared);
        let _ = axum::serve(listener, app).await;
    });
    Ok(())
}

async fn resource(State(shared): State<Arc<Shared>>, uri: Uri, headers: HeaderMap) -> Response {
    resource_response(&shared.assets, &uri, &headers)
}

fn resource_response(assets: &assets::Assets, uri: &Uri, headers: &HeaderMap) -> Response {
    let base = &assets.theme_base;
    let base_url = reqwest::Url::parse(base).expect("generated resource URL");
    let expected_host = format!("127.0.0.1:{}", base_url.port().unwrap());
    if headers.get("host").and_then(|s| s.to_str().ok()) != Some(expected_host.as_str()) {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new().into())
            .unwrap();
    }
    let Some(path) = uri.path().strip_prefix(base_url.path()) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new().into())
            .unwrap();
    };
    let path = percent_encoding::percent_decode_str(path).decode_utf8_lossy();
    let data = if path == "/sdk.js" {
        Ok(include_bytes!("../resources/theme-sdk.js").to_vec())
    } else if path.starts_with("/themes/") {
        assets.read_resource(&path)
    } else {
        Err("unknown resource".into())
    };
    let (status, mut data) = match data {
        Ok(data) => (200, data),
        Err(_) => (404, Vec::new()),
    };
    if status == 200 && path.ends_with(".html") {
        let html = String::from_utf8_lossy(&data);
        let script = format!("<script src=\"{base}/sdk.js\"></script>");
        data = if let Some(index) = html.to_ascii_lowercase().find("<head>") {
            format!("{}{}{}", &html[..index + 6], script, &html[index + 6..]).into_bytes()
        } else {
            format!("{script}{html}").into_bytes()
        };
    }
    let origin = base_url.origin().ascii_serialization();
    Response::builder().status(status)
        .header("Content-Type", assets::mime(&path))
        .header("X-Content-Type-Options", "nosniff")
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-store")
        .header("Content-Security-Policy", format!("default-src 'none'; script-src {origin} 'unsafe-inline'; style-src {origin} 'unsafe-inline'; img-src {origin} http://media.localhost data: blob:; font-src {origin}; connect-src 'none'; media-src 'none'; frame-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'; sandbox allow-scripts"))
        .body(data.into()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn serves_only_scoped_theme_files_with_sandbox_headers() {
        let root =
            std::env::temp_dir().join(format!("silverwolf-resource-{}", uuid::Uuid::new_v4()));
        let mut assets = assets::Assets::new(root.clone()).unwrap();
        assets.theme_base = "http://127.0.0.1:38431/random-scope".into();
        let id = assets
            .import_theme(vec![
                assets::ImportFile {
                    path: "theme.json".into(),
                    bytes: br#"{"schemaVersion":1,"name":"test","dialog":"dialog.html"}"#.to_vec(),
                },
                assets::ImportFile {
                    path: "dialog.html".into(),
                    bytes: b"<html><head></head><body>Hello</body></html>".to_vec(),
                },
            ])
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("host", "127.0.0.1:38431".parse().unwrap());
        let uri = format!("/random-scope/themes/{id}/dialog.html")
            .parse()
            .unwrap();
        let response = resource_response(&assets, &uri, &headers);
        assert_eq!(response.status(), StatusCode::OK);
        let csp = response.headers()["content-security-policy"]
            .to_str()
            .unwrap();
        assert!(csp.contains("connect-src 'none'") && csp.contains("sandbox allow-scripts"));
        assert!(!csp.contains("allow-same-origin"));
        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&body).contains("random-scope/sdk.js"));
        for path in [
            "/sdk.js",
            "/random-scope/http-token",
            "/random-scope/../settings.json",
            "/random-scope/themes/../../settings.json",
        ] {
            assert_eq!(
                resource_response(&assets, &path.parse().unwrap(), &headers).status(),
                StatusCode::NOT_FOUND
            );
        }
        headers.insert("host", "evil.example".parse().unwrap());
        assert_eq!(
            resource_response(&assets, &uri, &headers).status(),
            StatusCode::NOT_FOUND
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
