use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::{Client, Response};

use crate::error::DownloadError;
use crate::roster::{self, Platform};

const CONTENT_URL: &str =
    "https://eafc26.content.easports.com/fc/fltOnlineAssets/26E4D4D6-8DBB-4A9A-BD99-9C47D3AA341D/2026/";
const ROSTER_PATH: &str = "fc/fclive/genxtitle/rosterupdate.xml";

/// Download the latest PS4 squads file published by the EA roster manifest.
pub fn download_latest(output: &Path) -> Result<PathBuf, DownloadError> {
    let http = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(DownloadError::Client)?;
    let manifest_url = join_url(CONTENT_URL, ROSTER_PATH);
    let xml = fetch_text(&http, &manifest_url)?;
    let manifests = roster::parse_manifests(&xml)?;
    let manifest = roster::find(&manifests, Platform::Ps4)?;
    let relative = validate_relative_location(&manifest.major_location)?;
    let filename =
        relative.file_name().ok_or_else(|| DownloadError::UnsafePath(
            manifest.major_location.clone(),
        ))?;
    let destination = output
        .join("ps4")
        .join("squads")
        .join(&manifest.major_version)
        .join(filename);
    let squad_url = join_url(CONTENT_URL, &manifest.major_location);
    fetch_to_file(&http, &squad_url, &destination)?;
    Ok(destination)
}

/// Reject absolute paths and parent traversal in manifest locations.
pub fn validate_relative_location(
    location: &str,
) -> Result<&Path, DownloadError> {
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
    let response = send(http, url)?;
    response.text().map_err(|source| DownloadError::Body {
        url: url.to_owned(),
        source,
    })
}

fn send(http: &Client, url: &str) -> Result<Response, DownloadError> {
    match http.get(url).send() {
        Ok(response) if response.status().is_success() => Ok(response),
        Ok(response) => Err(DownloadError::Status {
            url: url.to_owned(),
            status: response.status(),
        }),
        Err(source) => Err(DownloadError::Request {
            url: url.to_owned(),
            source,
        }),
    }
}

fn fetch_to_file(
    http: &Client,
    url: &str,
    destination: &Path,
) -> Result<(), DownloadError> {
    let response = send(http, url)?;
    let bytes = response.bytes().map_err(|source| DownloadError::Body {
        url: url.to_owned(),
        source,
    })?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| DownloadError::Write {
            destination: parent.to_owned(),
            source,
        })?;
    }
    let temporary = destination.with_extension("part");
    fs::write(&temporary, &bytes).map_err(|source| DownloadError::Write {
        destination: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, destination).map_err(|source| {
        DownloadError::Write {
            destination: destination.to_owned(),
            source,
        }
    })?;
    Ok(())
}
