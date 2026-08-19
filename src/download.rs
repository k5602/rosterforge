use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::{Client, Response};

use crate::error::{DownloadError, RosterError};
use crate::roster::{self, Platform, SquadManifest};

pub const DEFAULT_CONTENT_URL: &str = "https://eafc26.content.easports.com/fc/fltOnlineAssets/26E4D4D6-8DBB-4A9A-BD99-9C47D3AA341D/2026/";
const ROSTER_PATH: &str = "fc/fclive/genxtitle/rosterupdate.xml";
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ATTEMPTS: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(2);

/// Which squad file of a manifest entry to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquadKind {
    /// The major squads file used by career saves.
    Major,
    /// The FUT squads file.
    Fut,
}

impl SquadKind {
    fn directory(self) -> &'static str {
        match self {
            SquadKind::Major => "squads",
            SquadKind::Fut => "futsquads",
        }
    }

    fn select(self, manifest: &SquadManifest) -> Result<(&str, &str), DownloadError> {
        match self {
            SquadKind::Major => Ok((&manifest.major_version, &manifest.major_location)),
            SquadKind::Fut => match (&manifest.fut_version, &manifest.fut_location) {
                (Some(version), Some(location)) => Ok((version, location)),
                _ => Err(DownloadError::Roster(RosterError::MissingField {
                    platform: manifest.platform.key(),
                    field: "dbFUTLoc",
                })),
            },
        }
    }
}

/// Download one squad file published by the EA roster manifest.
pub fn download_latest(
    output: &Path,
    content_url: &str,
    platform: Platform,
    kind: SquadKind,
) -> Result<PathBuf, DownloadError> {
    validate_content_url(content_url)?;
    let http = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(DownloadError::Client)?;
    let manifest_url = join_url(content_url, ROSTER_PATH);
    let xml = fetch_text(&http, &manifest_url)?;
    let manifests = roster::parse_manifests(&xml)?;
    let manifest = roster::find(&manifests, platform)?;
    let (version, location) = kind.select(manifest)?;
    let relative = validate_relative_location(location)?;
    let filename = relative
        .file_name()
        .ok_or_else(|| DownloadError::UnsafePath(location.to_owned()))?;
    let destination = output
        .join(platform.key())
        .join(kind.directory())
        .join(version)
        .join(filename);
    let squad_url = join_url(content_url, location);
    fetch_to_file(&http, &squad_url, &destination)?;
    Ok(destination)
}

/// Reject non-HTTPS content URLs so credentials and payloads stay encrypted.
pub fn validate_content_url(url: &str) -> Result<(), DownloadError> {
    if url.starts_with("https://") {
        Ok(())
    } else {
        Err(DownloadError::InsecureUrl)
    }
}

/// Reject absolute paths and parent traversal in manifest locations.
pub fn validate_relative_location(location: &str) -> Result<&Path, DownloadError> {
    let relative = Path::new(location);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(DownloadError::UnsafePath(location.to_owned()));
    }
    Ok(relative)
}

fn join_url(base: &str, relative: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), relative)
}

fn fetch_text(http: &Client, url: &str) -> Result<String, DownloadError> {
    let response = send_with_retry(http, url)?;
    response.text().map_err(|source| DownloadError::Body {
        url: url.to_owned(),
        source,
    })
}

fn send_with_retry(http: &Client, url: &str) -> Result<Response, DownloadError> {
    let mut attempt = 1;
    loop {
        match http.get(url).send() {
            Ok(response) if response.status().is_success() => {
                return Ok(response);
            }
            // Retry transient server failures and connection problems only.
            Ok(response) if response.status().is_server_error() && attempt < MAX_ATTEMPTS => {}
            Ok(response) => {
                return Err(DownloadError::Status {
                    url: url.to_owned(),
                    status: response.status(),
                });
            }
            Err(source)
                if attempt < MAX_ATTEMPTS && (source.is_timeout() || source.is_connect()) => {}
            Err(source) => {
                return Err(DownloadError::Request {
                    url: url.to_owned(),
                    source,
                });
            }
        }
        attempt += 1;
        std::thread::sleep(RETRY_DELAY);
    }
}

fn fetch_to_file(http: &Client, url: &str, destination: &Path) -> Result<(), DownloadError> {
    let response = send_with_retry(http, url)?;
    let total = response.content_length();
    if let Some(length) = total
        && length > MAX_DOWNLOAD_BYTES
    {
        return Err(DownloadError::TooLarge {
            url: url.to_owned(),
            limit: MAX_DOWNLOAD_BYTES,
        });
    }
    let progress = match total {
        Some(length) => ProgressBar::new(length),
        None => ProgressBar::new_spinner(),
    };
    progress.set_style(
        ProgressStyle::with_template("{spinner} [{bar:40.cyan/blue}] {bytes}/{total_bytes}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-"),
    );
    let temporary = destination.with_extension("part");
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| DownloadError::Write {
            destination: parent.to_owned(),
            source,
        })?;
    }
    let mut file = fs::File::create(&temporary).map_err(|source| DownloadError::Write {
        destination: temporary.clone(),
        source,
    })?;
    let mut reader = response;
    let mut buffer = [0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|source| DownloadError::Stream {
                url: url.to_owned(),
                source,
            })?;
        if count == 0 {
            break;
        }
        written += u64::try_from(count).unwrap_or(MAX_DOWNLOAD_BYTES + 1);
        if written > MAX_DOWNLOAD_BYTES {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err(DownloadError::TooLarge {
                url: url.to_owned(),
                limit: MAX_DOWNLOAD_BYTES,
            });
        }
        std::io::Write::write_all(&mut file, &buffer[..count]).map_err(|source| {
            DownloadError::Write {
                destination: temporary.clone(),
                source,
            }
        })?;
        progress.inc(u64::try_from(count).unwrap_or(0));
    }
    progress.finish_and_clear();
    drop(file);
    fs::rename(&temporary, destination).map_err(|source| DownloadError::Write {
        destination: destination.to_owned(),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_manifest_locations() {
        assert!(matches!(
            validate_relative_location("../etc/passwd"),
            Err(DownloadError::UnsafePath(_))
        ));
        assert!(matches!(
            validate_relative_location("/etc/passwd"),
            Err(DownloadError::UnsafePath(_))
        ));
        assert!(matches!(
            validate_relative_location("a/../../b"),
            Err(DownloadError::UnsafePath(_))
        ));
        assert!(validate_relative_location("fc/squads/file").is_ok());
    }

    #[test]
    fn rejects_insecure_content_urls() {
        assert!(matches!(
            validate_content_url("http://example.com/2026/"),
            Err(DownloadError::InsecureUrl)
        ));
        assert!(validate_content_url(DEFAULT_CONTENT_URL).is_ok());
    }
}
