//! Where releases come from: GitHub's REST API for the latest release (tag, notes, assets and their
//! digests), then the assets' download URLs. [`Feed`] is the seam the tests script.
//!
//! What GitHub answers (measured 2026-11-24 on this repository's v1.0.0): `GET
//! api.github.com/repos/{owner}/{repo}/releases/latest` gives `tag_name`, `html_url`, `body`, `draft`,
//! `prerelease` and per asset `name`, `size`, `browser_download_url` and `digest: "sha256:…"`; 60 requests
//! an hour per IP without a token. An asset's URL redirects to a signed `release-assets.githubusercontent.com`
//! URL valid for about an hour.

use std::io::{Read, Write};
use std::time::Duration;

use serde::Deserialize;

use super::{Fault, Release, Version};

/// The executable's asset name in a release.
pub const EXE_ASSET: &str = "quick-video-transcoder.exe";
/// The checksums' asset name.
pub const SUMS_ASSET: &str = "SHA256SUMS.txt";

/// A source of releases.
pub trait Feed: Send + Sync {
    /// The latest published release.
    fn latest(&self) -> Result<Release, Fault>;
    /// A small text file (the checksums).
    fn text(&self, url: &str) -> Result<String, Fault>;
    /// Streams a file into `out`, calling `progress` with the bytes written so far. Returns the size.
    fn download(
        &self,
        url: &str,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<u64, Fault>;
}

/// GitHub, over HTTPS.
pub struct GitHub {
    agent: ureq::Agent,
    /// `https://api.github.com` (a local listener in tests).
    api: String,
    /// `owner/repo`.
    repo: String,
}

impl GitHub {
    /// This application's own repository.
    pub fn new() -> Self {
        let repo = crate::build_info::REPOSITORY
            .trim_end_matches('/')
            .trim_start_matches("https://github.com/")
            .to_owned();
        Self::with_api("https://api.github.com", &repo)
    }

    /// Another API base, for the tests' listener.
    pub fn with_api(api: &str, repo: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            // Reading the whole executable: a slow line may need minutes, a dead one fails sooner.
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_recv_body(Some(Duration::from_secs(600)))
            // A generic client name: nothing about the user.
            .user_agent(format!(
                "quick-video-transcoder/{}",
                crate::build_info::VERSION
            ))
            .http_status_as_error(false)
            .build()
            .new_agent();
        Self {
            agent,
            api: api.trim_end_matches('/').to_owned(),
            repo: repo.to_owned(),
        }
    }

    fn get(&self, url: &str) -> Result<ureq::http::Response<ureq::Body>, Fault> {
        let response = self
            .agent
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .call()
            .map_err(|error| Fault::NoConnection(error.to_string()))?;
        let status = response.status().as_u16();
        if status == 200 {
            Ok(response)
        } else {
            log::warn!("update: {url} answered {status}");
            Err(Fault::Status(status))
        }
    }
}

impl Default for GitHub {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

/// Reads GitHub's answer for the latest release.
pub fn parse_release(json: &str) -> Result<Release, Fault> {
    let api: ApiRelease = serde_json::from_str(json)
        .map_err(|error| Fault::Unexpected(format!("not a release: {error}")))?;
    if api.draft || api.prerelease {
        return Err(Fault::Unexpected(format!(
            "{} is a draft or a pre-release",
            api.tag_name
        )));
    }
    let version = Version::from_tag(&api.tag_name)
        .ok_or_else(|| Fault::Unexpected(format!("tag {} is not vX.Y.Z", api.tag_name)))?;
    let asset = |name: &str| {
        api.assets
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| Fault::Unexpected(format!("{} has no {name}", api.tag_name)))
    };
    let exe = asset(EXE_ASSET)?;
    let sums = asset(SUMS_ASSET)?;
    let exe_digest = exe
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(str::to_ascii_lowercase);
    Ok(Release {
        version,
        page: api.html_url.clone(),
        notes: api.body.clone().unwrap_or_default(),
        exe_url: exe.browser_download_url.clone(),
        exe_size: exe.size,
        exe_digest,
        sums_url: sums.browser_download_url.clone(),
    })
}

impl Feed for GitHub {
    fn latest(&self) -> Result<Release, Fault> {
        let url = format!("{}/repos/{}/releases/latest", self.api, self.repo);
        let text = self.text(&url)?;
        parse_release(&text)
    }

    fn text(&self, url: &str) -> Result<String, Fault> {
        let mut response = self.get(url)?;
        response
            .body_mut()
            .with_config()
            .limit(1 << 20)
            .read_to_string()
            .map_err(|error| Fault::NoConnection(error.to_string()))
    }

    fn download(
        &self,
        url: &str,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<u64, Fault> {
        let mut response = self.get(url)?;
        let mut reader = response.body_mut().with_config().limit(512 << 20).reader();
        let mut buffer = vec![0u8; 256 << 10];
        let mut total = 0u64;
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| Fault::NoConnection(error.to_string()))?;
            if read == 0 {
                return Ok(total);
            }
            out.write_all(&buffer[..read])
                .map_err(|error| Fault::CannotWrite(error.to_string()))?;
            total += read as u64;
            progress(total);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    use super::*;

    const RELEASE: &str = r##"{
        "tag_name": "v1.1.0", "html_url": "https://github.com/o/r/releases/tag/v1.1.0",
        "body": "Added: languages", "draft": false, "prerelease": false,
        "assets": [
            {"name": "quick-video-transcoder.exe", "size": 5, "browser_download_url": "URL/exe",
             "digest": "sha256:2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"},
            {"name": "SHA256SUMS.txt", "size": 92, "browser_download_url": "URL/sums", "digest": null}
        ]
    }"##;

    #[test]
    fn reads_the_latest_release() {
        let release = parse_release(RELEASE).unwrap();
        assert_eq!(release.version, Version(1, 1, 0));
        assert_eq!(release.exe_url, "URL/exe");
        assert_eq!(release.sums_url, "URL/sums");
        assert_eq!(
            release.exe_digest.as_deref(),
            Some("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
        );
        assert!(release.notes.contains("languages"));
    }

    #[test]
    fn refuses_what_it_does_not_understand() {
        for (from, to) in [
            ("v1.1.0", "v1.1"),
            ("v1.1.0", "1.1.0"),
            ("\"draft\": false", "\"draft\": true"),
            ("\"prerelease\": false", "\"prerelease\": true"),
            ("quick-video-transcoder.exe", "other.exe"),
            ("SHA256SUMS.txt", "sums.txt"),
        ] {
            let json = RELEASE.replace(from, to);
            assert!(
                matches!(parse_release(&json), Err(Fault::Unexpected(_))),
                "{from} -> {to}"
            );
        }
        assert!(matches!(
            parse_release("<html>captive portal</html>"),
            Err(Fault::Unexpected(_))
        ));
    }

    /// A one-shot HTTP server on 127.0.0.1 answering each request with the next canned response.
    fn serve(responses: Vec<(u16, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn talks_http_and_reports_statuses() {
        let base = serve(vec![
            (200, RELEASE.to_owned()),
            (503, "busy".to_owned()),
            (200, "hello".to_owned()),
        ]);
        let feed = GitHub::with_api(&base, "o/r");
        assert_eq!(feed.latest().unwrap().version, Version(1, 1, 0));
        assert_eq!(feed.text(&format!("{base}/x")), Err(Fault::Status(503)));
        let mut out = Vec::new();
        let mut seen = 0;
        let size = feed
            .download(&format!("{base}/exe"), &mut out, &mut |n| seen = n)
            .unwrap();
        assert_eq!((size, seen, out.as_slice()), (5, 5, b"hello".as_slice()));
    }

    #[test]
    fn a_port_nobody_listens_on_is_no_connection() {
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = free.local_addr().unwrap();
        drop(free);
        let feed = GitHub::with_api(&format!("http://{address}"), "o/r");
        assert!(matches!(feed.latest(), Err(Fault::NoConnection(_))));
    }
}
