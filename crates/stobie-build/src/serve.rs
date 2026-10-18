//! A local development server for a built web root: loopback only, static
//! files with the right media types and `Cache-Control: no-store`, so a
//! reload always sees the latest build. It is not a production server.

use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};

use crate::Error;

/// The media type for a file name, or `application/octet-stream`.
pub fn media_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("webmanifest") => "application/manifest+json",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("ttf") => "font/ttf",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// The file under `root` for a request target, refusing anything that could
/// leave it. A directory maps to its `index.html`.
pub fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next()?;
    let decoded = percent_decode(path)?;
    if !decoded.starts_with('/') {
        return None;
    }
    let mut resolved = root.to_path_buf();
    for component in Path::new(&decoded[1..]).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if resolved.is_dir() {
        resolved.push("index.html");
    }
    Some(resolved)
}

/// The location to redirect a directory request without its trailing slash to.
fn directory_redirect(root: &Path, target: &str) -> Option<String> {
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (target, None),
    };
    if path.ends_with('/') {
        return None;
    }
    let mut resolved = root.to_path_buf();
    for component in Path::new(percent_decode(path)?.strip_prefix('/')?).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    resolved.is_dir().then(|| {
        format!(
            "{path}/{}",
            query.map(|query| format!("?{query}")).unwrap_or_default()
        )
    })
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            output.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(output).ok()?;
    (!decoded.contains('\\') && !decoded.contains('\0')).then_some(decoded)
}

fn respond(stream: &mut TcpStream, status: &str, media: &str, body: &[u8], head: bool) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {media}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    if !head {
        let _ = stream.write_all(body);
    }
}

fn handle(root: &Path, mut stream: TcpStream) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => return,
    });
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() {
        return;
    }
    // Drain the headers; nothing in them changes the response.
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
        line.clear();
    }
    let mut parts = request.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let head = method == "HEAD";
    if method != "GET" && !head {
        respond(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"",
            head,
        );
        return;
    }
    // A directory's relative URLs resolve only below a trailing slash, so
    // `/app` redirects to `/app/` (keeping the query) before serving its index.
    if let Some(location) = directory_redirect(root, target) {
        let header = format!(
            "HTTP/1.1 308 Permanent Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.write_all(header.as_bytes());
        return;
    }
    let Some(path) = resolve(root, target) else {
        respond(
            &mut stream,
            "400 Bad Request",
            "text/plain",
            b"bad request\n",
            head,
        );
        return;
    };
    match fs::read(&path) {
        Ok(body) => respond(&mut stream, "200 OK", media_type(&path), &body, head),
        Err(_) => respond(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"not found\n",
            head,
        ),
    }
}

/// Bind `address` for `root`, failing early unless it is a web root (with
/// `index.html`) or a production package (with `manifest.json`, whose
/// applications are under `/NAME/`).
pub fn bind(root: &Path, address: SocketAddr) -> Result<TcpListener, Error> {
    if !root.join("index.html").is_file() && !root.join("manifest.json").is_file() {
        return Err(Error::new(
            format!(
                "{} has no index.html or package manifest.json",
                root.display()
            ),
            "run `stobie build-web` first, or pass the web root or production package to serve",
        ));
    }
    TcpListener::bind(address).map_err(|error| {
        Error::new(
            format!("cannot listen on {address}: {error}"),
            "choose another --port",
        )
    })
}

/// Serve `root` on `listener` until the process ends, one thread per
/// connection.
pub fn run(root: &Path, listener: TcpListener) {
    for stream in listener.incoming().flatten() {
        let root = root.to_path_buf();
        std::thread::spawn(move || handle(&root, stream));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_stay_inside_the_root() {
        let root = Path::new("/srv/web");
        assert_eq!(
            resolve(root, "/pkg/app.js?x=1"),
            Some(PathBuf::from("/srv/web/pkg/app.js"))
        );
        assert_eq!(
            resolve(root, "/a%20b.js"),
            Some(PathBuf::from("/srv/web/a b.js"))
        );
        for escape in [
            "/../etc/passwd",
            "/%2e%2e/secret",
            "/a/%5c..%5cb",
            "relative",
            "/%ZZ",
        ] {
            assert_eq!(resolve(root, escape), None, "{escape}");
        }
        assert_eq!(media_type(Path::new("x/app_bg.wasm")), "application/wasm");
        assert_eq!(
            media_type(Path::new("bootstrap.js")),
            "text/javascript; charset=utf-8"
        );
    }

    #[test]
    fn serves_files_with_no_store_and_rejects_other_methods() {
        let directory = std::env::temp_dir().join(format!("stobie-serve-{}", std::process::id()));
        fs::create_dir_all(directory.join("pkg")).unwrap();
        fs::write(directory.join("index.html"), "<html></html>").unwrap();
        fs::write(directory.join("pkg/app_bg.wasm"), [0, 97, 115, 109]).unwrap();
        let listener = bind(&directory, "127.0.0.1:0".parse().unwrap()).unwrap();
        let address = listener.local_addr().unwrap();
        let root = directory.clone();
        std::thread::spawn(move || run(&root, listener));
        let get = |request: &str| {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            let mut response = Vec::new();
            std::io::Read::read_to_end(&mut stream, &mut response).unwrap();
            String::from_utf8_lossy(&response).into_owned()
        };
        let index = get("GET / HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(index.starts_with("HTTP/1.1 200 OK"), "{index}");
        assert!(index.contains("Content-Type: text/html"));
        assert!(index.contains("Cache-Control: no-store"));
        assert!(index.ends_with("<html></html>"));
        let wasm = get("GET /pkg/app_bg.wasm HTTP/1.1\r\n\r\n");
        assert!(wasm.contains("Content-Type: application/wasm"));
        assert!(get("GET /missing.js HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 404"));
        assert!(get("GET /../x HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 400"));
        assert!(get("POST / HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 405"));
        fs::create_dir_all(directory.join("app")).unwrap();
        fs::write(directory.join("app/index.html"), "app").unwrap();
        let redirect = get("GET /app?automation=1 HTTP/1.1\r\n\r\n");
        assert!(redirect.starts_with("HTTP/1.1 308"), "{redirect}");
        assert!(redirect.contains("Location: /app/?automation=1\r\n"));
        assert!(get("GET /app/ HTTP/1.1\r\n\r\n").ends_with("app"));
        // A production package is served from its root; an empty directory is not.
        let package = directory.join("package");
        fs::create_dir_all(package.join("app")).unwrap();
        fs::write(package.join("manifest.json"), "{}").unwrap();
        assert!(bind(&package, "127.0.0.1:0".parse().unwrap()).is_ok());
        let empty = directory.join("empty");
        fs::create_dir_all(&empty).unwrap();
        assert!(bind(&empty, "127.0.0.1:0".parse().unwrap()).is_err());
        fs::remove_dir_all(&directory).unwrap();
    }
}
