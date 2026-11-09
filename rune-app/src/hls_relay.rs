//! A loopback relay for music video streams. GStreamer plays
//! `http://127.0.0.1:<port>/<token>/https/<host>/<path>` and this fetches
//! `https://<host>/<path>` with the backend's shared HTTP client
//! (`AppState::proxied_http`, as artwork does), so the user's proxy applies to video exactly
//! as to everything else. When the proxy plan blocks traffic the client is refused here, and
//! GStreamer itself never opens a connection to the internet, so it can't go around one.
//!
//! HLS playlists are rewritten on the way: every URI in them (segment and variant lines, and
//! `URI="…"` attributes) is resolved against the playlist's final address (after redirects) and
//! pointed back at the relay. Media bytes stream through unchanged, `Range` requests included.
//! The listener is loopback-only and every path must carry a random per-run token.

use std::sync::OnceLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tauri_app_lib::AppState;
use sone_host_shim::Manager;

static BASE: OnceLock<Result<String, String>> = OnceLock::new();

fn base() -> Result<String, String> {
    BASE.get_or_init(|| {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("relay: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| format!("relay: {e}"))?;
        let port = listener.local_addr().map_err(|e| format!("relay: {e}"))?.port();
        let token = format!("{:032x}", rand::random::<u128>());
        let t = token.clone();
        sone_host_shim::async_runtime::spawn(async move {
            let Ok(listener) = tokio::net::TcpListener::from_std(listener) else { return };
            loop {
                let Ok((sock, _)) = listener.accept().await else { continue };
                let t = t.clone();
                sone_host_shim::async_runtime::spawn(async move {
                    if let Err(e) = serve(sock, &t).await {
                        log::debug!("[video relay] {e}");
                    }
                });
            }
        });
        Ok(format!("http://127.0.0.1:{port}/{token}"))
    })
    .clone()
}

/// The relay address for `url` (https or http); anything else is returned as it is.
pub fn wrap(url: &str) -> Result<String, String> {
    let base = base()?;
    Ok(relay_form(&base, url))
}

fn relay_form(base: &str, url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        format!("{base}/https/{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("{base}/http/{rest}")
    } else {
        url.to_string()
    }
}

/// Every URI in an M3U8 playlist, resolved against `resolve` and sent back through the relay.
fn rewrite_playlist(text: &str, base: &str, resolve: impl Fn(&str) -> Option<String>) -> String {
    let fix = |u: &str| resolve(u).map(|abs| relay_form(base, &abs)).unwrap_or_else(|| u.to_string());
    let mut out = String::with_capacity(text.len() + 256);
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            out.push_str(line);
        } else if !l.starts_with('#') {
            out.push_str(&fix(l));
        } else if let Some(i) = l.find("URI=\"") {
            let start = i + 5;
            match l[start..].find('"') {
                Some(len) => {
                    out.push_str(&l[..start]);
                    out.push_str(&fix(&l[start..start + len]));
                    out.push_str(&l[start + len..]);
                }
                None => out.push_str(line),
            }
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

async fn respond(sock: &mut TcpStream, status: u16, reason: &str) -> std::io::Result<()> {
    sock.write_all(format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await
}

async fn serve(mut sock: TcpStream, token: &str) -> std::io::Result<()> {
    // The request head (GStreamer sends a small one).
    let mut head = Vec::with_capacity(2048);
    let mut buf = [0u8; 2048];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = sock.read(&mut buf).await?;
        if n == 0 || head.len() > 32 * 1024 {
            return Ok(());
        }
        head.extend_from_slice(&buf[..n]);
    }
    let head = String::from_utf8_lossy(&head).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    if method != "GET" && method != "HEAD" {
        return respond(&mut sock, 405, "Method Not Allowed").await;
    }
    let range = lines.filter_map(|l| l.split_once(':')).find(|(k, _)| k.trim().eq_ignore_ascii_case("range")).map(|(_, v)| v.trim().to_string());
    let Some(rest) = path.strip_prefix('/').and_then(|p| p.strip_prefix(token)).and_then(|p| p.strip_prefix('/')) else {
        return respond(&mut sock, 403, "Forbidden").await;
    };
    let upstream = match rest.split_once('/') {
        Some(("https", r)) => format!("https://{r}"),
        Some(("http", r)) => format!("http://{r}"),
        _ => return respond(&mut sock, 400, "Bad Request").await,
    };

    let Ok(client) = crate::host().state::<AppState>().proxied_http.client() else {
        // The proxy plan blocks traffic: no video, as no artwork.
        return respond(&mut sock, 502, "Bad Gateway").await;
    };
    let mut req = if method == "HEAD" { client.head(&upstream) } else { client.get(&upstream) };
    if let Some(r) = &range {
        req = req.header("Range", r.as_str());
    }
    let mut resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            log::debug!("[video relay] {upstream}: {e}");
            return respond(&mut sock, 502, "Bad Gateway").await;
        }
    };
    let status = resp.status();
    let header = |k: &str| resp.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    let ctype = header("content-type").unwrap_or_else(|| "application/octet-stream".into());
    let is_playlist = ctype.contains("mpegurl") || upstream.split('?').next().unwrap_or("").ends_with(".m3u8");
    let mut out = format!("HTTP/1.1 {} {}\r\nContent-Type: {ctype}\r\nConnection: close\r\n", status.as_u16(), status.canonical_reason().unwrap_or(""));

    if is_playlist && status.is_success() {
        let final_url = resp.url().clone();
        let text = resp.text().await.unwrap_or_default();
        let base = base().unwrap_or_default();
        let body = rewrite_playlist(&text, &base, |u| final_url.join(u).ok().map(|x| x.to_string()));
        out.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
        sock.write_all(out.as_bytes()).await?;
        if method == "GET" {
            sock.write_all(body.as_bytes()).await?;
        }
        return Ok(());
    }
    for k in ["content-length", "content-range", "accept-ranges"] {
        if let Some(v) = header(k) {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
    }
    out.push_str("\r\n");
    sock.write_all(out.as_bytes()).await?;
    if method == "GET" {
        // Verifier only (video_check.py --close-test): segments trickle out, so a download is
        // mid-body when the video closes.
        let slow = std::env::var_os("SPIKE_SLOW_TS").is_some() && upstream.split('?').next().unwrap_or("").ends_with(".ts");
        while let Ok(Some(chunk)) = resp.chunk().await {
            if slow {
                for part in chunk.chunks(16 * 1024) {
                    sock.write_all(part).await?;
                    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                }
            } else {
                sock.write_all(&chunk).await?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlists_point_back_at_the_relay() {
        let base = "http://127.0.0.1:1/tok";
        let pl = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"https://k.example/key\"\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv720/index.m3u8\nhttps://cdn.example/a/b.ts?x=1\n";
        let out = rewrite_playlist(pl, base, |u| {
            if u.starts_with("http") { Some(u.to_string()) } else { Some(format!("https://cdn.example/root/{u}")) }
        });
        assert!(out.contains("URI=\"http://127.0.0.1:1/tok/https/k.example/key\""));
        assert!(out.contains("\nhttp://127.0.0.1:1/tok/https/cdn.example/root/v720/index.m3u8\n"));
        assert!(out.contains("\nhttp://127.0.0.1:1/tok/https/cdn.example/a/b.ts?x=1\n"));
        assert!(out.starts_with("#EXTM3U\n"));
    }
}
