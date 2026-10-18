//! Installable web application output. A page that links
//! `manifest.webmanifest` asks for it: builds then write that web app
//! manifest and a service worker, `sw.js`, at the web root beside
//! `index.html`, never in a production package's hashed asset directory.
//!
//! The page's own head is the single source of the application's identity,
//! because iOS reads it there: `<title>` (or `application-name`) names it,
//! `apple-mobile-web-app-title` gives the short name, `theme-color` the colour,
//! and `<link rel="icon" sizes>` and `<link rel="apple-touch-icon">` the icons.
//! The manifest is derived from those tags.

use std::fs;
use std::ops::Range;
use std::path::Path;

use crate::Error;

/// The web app manifest's file name at the web root.
pub const MANIFEST_FILE: &str = "manifest.webmanifest";
/// The service worker's file name at the web root.
pub const SERVICE_WORKER_FILE: &str = "sw.js";
const SERVICE_WORKER: &str = include_str!("../web/service-worker.js");

/// The icon sizes a manifest needs, in pixels: 192 and 512 for installation,
/// and 180 for iOS's `apple-touch-icon`.
pub const ICON_SIZES: [u32; 2] = [192, 512];
pub const APPLE_TOUCH_ICON_SIZE: u32 = 180;

/// An installable page's identity, read from its head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub short_name: String,
    pub description: Option<String>,
    pub theme_color: String,
    pub background_color: String,
    /// `rel="icon"` links with one square `sizes` value.
    pub icons: Vec<Icon>,
    pub apple_touch_icon: String,
    /// The bytes of the manifest link's `href` value in the page.
    manifest_href: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Icon {
    pub href: String,
    pub size: u32,
}

#[derive(Debug)]
struct Attribute {
    name: String,
    value: String,
    span: Range<usize>,
}

#[derive(Debug)]
struct Tag {
    name: String,
    attributes: Vec<Attribute>,
    /// The text of a `<title>`.
    text: String,
}

impl Tag {
    fn attribute(&self, name: &str) -> Option<&Attribute> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.attribute(name)
            .map(|attribute| attribute.value.as_str())
    }

    /// Whether a space-separated attribute such as `rel` contains `token`.
    fn has_token(&self, name: &str, token: &str) -> bool {
        self.value(name).is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|part| part.eq_ignore_ascii_case(token))
        })
    }
}

fn decode(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn find_ignore_case(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    haystack[from..]
        .to_ascii_lowercase()
        .find(needle)
        .map(|index| from + index)
}

/// The start tags of the document's head, in order, with their attributes.
/// Comments are skipped, and `script`, `style` and `title` content is read as
/// text. Scanning stops at `</head>` or `<body>`.
fn head_tags(html: &str) -> Vec<Tag> {
    let bytes = html.as_bytes();
    let mut tags = Vec::new();
    let mut index = 0;
    while let Some(offset) = html[index..].find('<') {
        index += offset;
        let rest = &html[index..];
        if rest.starts_with("<!--") {
            index = html[index..]
                .find("-->")
                .map_or(html.len(), |end| index + end + 3);
            continue;
        }
        let closing = rest.starts_with("</");
        let start = index + if closing { 2 } else { 1 };
        let name_end = html[start..]
            .find(|c: char| !c.is_ascii_alphanumeric())
            .map_or(html.len(), |end| start + end);
        let name = html[start..name_end].to_ascii_lowercase();
        if name.is_empty() {
            index += 1;
            continue;
        }
        if closing {
            if name == "head" {
                break;
            }
            index = html[name_end..]
                .find('>')
                .map_or(html.len(), |end| name_end + end + 1);
            continue;
        }
        if name == "body" {
            break;
        }
        // Attributes, up to the tag's end.
        let mut attributes = Vec::new();
        let mut cursor = name_end;
        loop {
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor >= bytes.len() {
                break;
            }
            if bytes[cursor] == b'>' {
                cursor += 1;
                break;
            }
            if bytes[cursor] == b'/' {
                cursor += 1;
                continue;
            }
            let attribute_start = cursor;
            while cursor < bytes.len()
                && !bytes[cursor].is_ascii_whitespace()
                && !matches!(bytes[cursor], b'=' | b'>' | b'/')
            {
                cursor += 1;
            }
            let attribute = html[attribute_start..cursor].to_ascii_lowercase();
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            let (value, span) = if cursor < bytes.len() && bytes[cursor] == b'=' {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                match bytes.get(cursor) {
                    Some(&quote @ (b'"' | b'\'')) => {
                        let value_start = cursor + 1;
                        let value_end = html[value_start..]
                            .find(quote as char)
                            .map_or(html.len(), |end| value_start + end);
                        cursor = (value_end + 1).min(html.len());
                        (&html[value_start..value_end], value_start..value_end)
                    }
                    _ => {
                        let value_start = cursor;
                        while cursor < bytes.len()
                            && !bytes[cursor].is_ascii_whitespace()
                            && bytes[cursor] != b'>'
                        {
                            cursor += 1;
                        }
                        (&html[value_start..cursor], value_start..cursor)
                    }
                }
            } else {
                ("", cursor..cursor)
            };
            if !attribute.is_empty() {
                attributes.push(Attribute {
                    name: attribute,
                    value: decode(value),
                    span,
                });
            }
        }
        let mut text = String::new();
        if matches!(name.as_str(), "script" | "style" | "title") {
            let end = find_ignore_case(html, &format!("</{name}"), cursor).unwrap_or(html.len());
            if name == "title" {
                text = decode(html[cursor..end].trim());
            }
            cursor = end;
        }
        tags.push(Tag {
            name,
            attributes,
            text,
        });
        index = cursor;
    }
    tags
}

fn square_size(sizes: &str) -> Option<u32> {
    let mut sizes = sizes.split_ascii_whitespace();
    let (width, height) = sizes
        .next()?
        .to_ascii_lowercase()
        .split_once('x')
        .map(|(width, height)| (width.parse::<u32>().ok(), height.parse::<u32>().ok()))?;
    match (width?, height?, sizes.next()) {
        (width, height, None) if width == height => Some(width),
        _ => None,
    }
}

fn missing(what: &str, tag: &str) -> Error {
    Error::new(
        format!("index.html links {MANIFEST_FILE} but has no {what}"),
        format!("add {tag} to its <head>"),
    )
}

/// Read the page's installable identity, or `None` when it links no manifest.
pub fn declaration(html: &str) -> Result<Option<Declaration>, Error> {
    let tags = head_tags(html);
    let Some(link) = tags
        .iter()
        .find(|tag| tag.name == "link" && tag.has_token("rel", "manifest"))
    else {
        return Ok(None);
    };
    let href = link
        .attribute("href")
        .filter(|href| href.value == MANIFEST_FILE)
        .ok_or_else(|| {
            Error::new(
                format!("the manifest link must name {MANIFEST_FILE} beside index.html"),
                format!("use <link rel=\"manifest\" href=\"{MANIFEST_FILE}\">"),
            )
        })?;
    let meta = |name: &str| {
        tags.iter()
            .filter(|tag| {
                tag.name == "meta"
                    && tag
                        .value("name")
                        .is_some_and(|value| value.eq_ignore_ascii_case(name))
            })
            .collect::<Vec<_>>()
    };
    let content = |name: &str| {
        meta(name)
            .first()
            .and_then(|tag| tag.value("content"))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let viewport = content("viewport").unwrap_or_default();
    if !viewport
        .split(',')
        .any(|part| part.trim().eq_ignore_ascii_case("viewport-fit=cover"))
    {
        return Err(missing(
            "viewport-fit=cover in its viewport",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">",
        ));
    }
    let title = tags
        .iter()
        .find(|tag| tag.name == "title")
        .map(|tag| tag.text.clone())
        .filter(|text| !text.is_empty());
    let name = content("application-name")
        .or(title)
        .ok_or_else(|| missing("name", "a <title>"))?;
    let short_name = content("apple-mobile-web-app-title").ok_or_else(|| {
        missing(
            "short name",
            "<meta name=\"apple-mobile-web-app-title\" content=\"...\">",
        )
    })?;
    // A colour for every scheme comes first; otherwise the first given.
    let themes = meta("theme-color");
    let theme_color = themes
        .iter()
        .find(|tag| tag.value("media").is_none())
        .or(themes.first())
        .and_then(|tag| tag.value("content"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            missing(
                "theme colour",
                "<meta name=\"theme-color\" content=\"#...\">",
            )
        })?;
    let background_color =
        content("stobie-background-color").unwrap_or_else(|| theme_color.clone());
    let icons: Vec<Icon> = tags
        .iter()
        .filter(|tag| tag.name == "link" && tag.has_token("rel", "icon"))
        .filter_map(|tag| {
            Some(Icon {
                href: tag.value("href")?.to_owned(),
                size: square_size(tag.value("sizes")?)?,
            })
        })
        .collect();
    for size in ICON_SIZES {
        if !icons.iter().any(|icon| icon.size == size) {
            return Err(missing(
                &format!("{size}×{size} icon"),
                &format!(
                    "<link rel=\"icon\" type=\"image/png\" sizes=\"{size}x{size}\" href=\"icons/icon-{size}.png\">"
                ),
            ));
        }
    }
    let apple_touch_icon = tags
        .iter()
        .find(|tag| tag.name == "link" && tag.has_token("rel", "apple-touch-icon"))
        .and_then(|tag| tag.value("href"))
        .map(str::to_owned)
        .ok_or_else(|| {
            missing(
                "apple-touch-icon",
                "<link rel=\"apple-touch-icon\" href=\"icons/apple-touch-icon.png\">",
            )
        })?;
    Ok(Some(Declaration {
        name,
        short_name,
        description: content("description"),
        theme_color,
        background_color,
        icons,
        apple_touch_icon,
        manifest_href: href.span.clone(),
    }))
}

/// The width and height of a PNG file, from its header.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let read = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("four bytes"));
    Some((read(16), read(20)))
}

impl Declaration {
    /// Every icon with the size it must have.
    fn sized_icons(&self) -> impl Iterator<Item = (&str, u32)> {
        self.icons
            .iter()
            .filter(|icon| ICON_SIZES.contains(&icon.size))
            .map(|icon| (icon.href.as_str(), icon.size))
            .chain(std::iter::once((
                self.apple_touch_icon.as_str(),
                APPLE_TOUCH_ICON_SIZE,
            )))
    }

    /// Check that each icon the manifest names is a square PNG of its size
    /// inside `web_root`.
    pub fn check_icons(&self, web_root: &Path) -> Result<(), Error> {
        for (href, size) in self.sized_icons() {
            let relative = Path::new(href);
            let inside = !href.is_empty()
                && !href.contains(':')
                && !href.contains(['?', '#', '\\'])
                && relative
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)));
            if !inside {
                return Err(Error::new(
                    format!("icon {href:?} is not a file inside the web root"),
                    "give icons relative paths such as icons/icon-192.png",
                ));
            }
            let path = web_root.join(relative);
            let bytes = fs::read(&path).map_err(|error| {
                Error::new(
                    format!("icon {}: {error}", path.display()),
                    "add the PNG icons the page names; `stobie new` writes a set to replace",
                )
            })?;
            match png_size(&bytes) {
                Some((width, height)) if width == size && height == size => {}
                Some((width, height)) => {
                    return Err(Error::new(
                        format!("icon {href} is {width}×{height}, not {size}×{size}"),
                        format!("give it a {size}×{size} PNG"),
                    ));
                }
                None => {
                    return Err(Error::new(
                        format!("icon {href} is not a PNG"),
                        format!("give it a {size}×{size} PNG"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// The web app manifest, whose icon paths start with `prefix`: empty
    /// beside the page, or a production package's asset directory.
    pub fn manifest(&self, prefix: &str) -> String {
        let mut icons: Vec<serde_json::Value> = Vec::new();
        for size in ICON_SIZES {
            if let Some(icon) = self.icons.iter().find(|icon| icon.size == size) {
                icons.push(serde_json::json!({
                    "src": format!("{prefix}{}", icon.href),
                    "sizes": format!("{size}x{size}"),
                    "type": "image/png",
                    "purpose": "any",
                }));
            }
        }
        icons.push(serde_json::json!({
            "src": format!("{prefix}{}", self.apple_touch_icon),
            "sizes": format!("{APPLE_TOUCH_ICON_SIZE}x{APPLE_TOUCH_ICON_SIZE}"),
            "type": "image/png",
        }));
        let mut manifest = serde_json::json!({
            "id": "./",
            "name": self.name,
            "short_name": self.short_name,
            "start_url": "./",
            "scope": "./",
            "display": "standalone",
            "background_color": self.background_color,
            "theme_color": self.theme_color,
            "icons": icons,
        });
        if let Some(description) = &self.description {
            manifest["description"] = description.clone().into();
        }
        let mut text = serde_json::to_string_pretty(&manifest).expect("a manifest serialises");
        text.push('\n');
        text
    }

    /// The page with its manifest link pointing at `href` instead.
    pub fn with_manifest_href(&self, html: &str, href: &str) -> String {
        format!(
            "{}{href}{}",
            &html[..self.manifest_href.start],
            &html[self.manifest_href.end..]
        )
    }
}

/// The service worker for `app` at `version`, which installs with every
/// path in `precache`, relative to its scope, already cached.
pub fn service_worker(app: &str, version: &str, precache: &[String]) -> String {
    SERVICE_WORKER
        .replace(
            "__STOBIE_APP__",
            &serde_json::to_string(app).expect("a string serialises"),
        )
        .replace(
            "__STOBIE_VERSION__",
            &serde_json::to_string(version).expect("a string serialises"),
        )
        .replace(
            "__STOBIE_PRECACHE__",
            &serde_json::to_string(precache).expect("strings serialise"),
        )
}

/// A placeholder icon: a disc of `foreground` centred on `background`, as a
/// `size`×`size` PNG. `stobie new` writes a set for the application to replace.
pub fn placeholder_icon(size: u32, background: [u8; 3], foreground: [u8; 3]) -> Vec<u8> {
    use std::io::Write as _;
    let centre = size as f32 / 2.0;
    let radius = size as f32 * 0.3;
    let mut pixels = Vec::with_capacity((size * (size * 3 + 1)) as usize);
    for y in 0..size {
        // Each row starts with its filter type, none.
        pixels.push(0);
        for x in 0..size {
            // Four samples per pixel smooth the edge.
            let mut covered = 0.0;
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let (px, py) = (x as f32 + dx - centre, y as f32 + dy - centre);
                if px * px + py * py <= radius * radius {
                    covered += 0.25;
                }
            }
            for channel in 0..3 {
                let mixed = background[channel] as f32 * (1.0 - covered)
                    + foreground[channel] as f32 * covered;
                pixels.push(mixed.round() as u8);
            }
        }
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(&pixels).expect("in-memory zlib");
    let data = encoder.finish().expect("in-memory zlib");
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&size.to_be_bytes());
    // Eight-bit RGB, deflate, adaptive filtering, no interlace.
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let chunks: [(&[u8], &[u8]); 3] = [(b"IHDR", &header), (b"IDAT", &data), (b"IEND", &[])];
    for (kind, body) in chunks {
        png.extend_from_slice(&(body.len() as u32).to_be_bytes());
        png.extend_from_slice(kind);
        png.extend_from_slice(body);
        let mut crc = flate2::Crc::new();
        crc.update(kind);
        crc.update(body);
        png.extend_from_slice(&crc.sum().to_be_bytes());
    }
    png
}

#[cfg(test)]
pub(crate) mod tests;
