use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::RosterError;

/// Squad platforms published in the EA roster manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Ps4,
    Xbox,
}

impl Platform {
    pub const ALL: [Platform; 2] = [Platform::Ps4, Platform::Xbox];

    /// Manifest attribute key for this platform.
    pub fn key(self) -> &'static str {
        match self {
            Platform::Ps4 => "ps4",
            Platform::Xbox => "xbox",
        }
    }

    fn from_key(value: &[u8]) -> Option<Platform> {
        Platform::ALL
            .into_iter()
            .find(|platform| platform.key().as_bytes().eq_ignore_ascii_case(value))
    }
}

/// One `squadInfo` entry of the EA roster manifest.
#[derive(Debug, Clone)]
pub struct SquadManifest {
    pub platform: Platform,
    pub major_version: String,
    pub major_location: String,
    pub fut_version: Option<String>,
    pub fut_location: Option<String>,
}

/// Parse every supported `squadInfo` entry from a `rosterupdate.xml` document.
pub fn parse_manifests(xml: &str) -> Result<Vec<SquadManifest>, RosterError> {
    let mut reader = Reader::from_str(xml);
    let mut manifests = Vec::new();
    let mut current: Option<Entry> = None;
    let mut element = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = event.name();
                let name_bytes: &[u8] = name.as_ref();
                if name_bytes.eq_ignore_ascii_case(b"squadInfo") {
                    current = event.attributes().flatten().find_map(|attr| {
                        if !attr.key.as_ref().eq_ignore_ascii_case(b"platform") {
                            return None;
                        }
                        Platform::from_key(&attr.value).map(|platform| Entry {
                            platform,
                            major_version: None,
                            major_location: None,
                            fut_version: None,
                            fut_location: None,
                        })
                    });
                }
                element = String::from_utf8_lossy(name_bytes).to_string();
            }
            Ok(Event::Text(text)) => {
                let Some(entry) = current.as_mut() else {
                    continue;
                };
                let value = text
                    .decode()
                    .map_err(|error| RosterError::Xml(error.to_string()))?
                    .trim()
                    .to_owned();
                let name = element.as_str();
                if name.eq_ignore_ascii_case("dbMajor") {
                    entry.major_version = Some(value);
                } else if name.eq_ignore_ascii_case("dbMajorLoc") {
                    entry.major_location = Some(value);
                } else if name.eq_ignore_ascii_case("dbFUTVer") {
                    entry.fut_version = Some(value);
                } else if name.eq_ignore_ascii_case("dbFUTLoc") {
                    entry.fut_location = Some(value);
                }
            }
            Ok(Event::End(event)) => {
                if event.name().as_ref().eq_ignore_ascii_case(b"squadInfo")
                    && let Some(entry) = current.take()
                {
                    manifests.push(entry.finish()?);
                }
                element.clear();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(RosterError::Xml(error.to_string())),
        }
    }
    Ok(manifests)
}

/// Find the manifest entry for one platform.
pub fn find(
    manifests: &[SquadManifest],
    platform: Platform,
) -> Result<&SquadManifest, RosterError> {
    manifests
        .iter()
        .find(|manifest| manifest.platform == platform)
        .ok_or(RosterError::MissingPlatform {
            platform: platform.key(),
        })
}

struct Entry {
    platform: Platform,
    major_version: Option<String>,
    major_location: Option<String>,
    fut_version: Option<String>,
    fut_location: Option<String>,
}

impl Entry {
    fn finish(self) -> Result<SquadManifest, RosterError> {
        let platform = self.platform;
        Ok(SquadManifest {
            platform,
            major_version: self.major_version.ok_or(RosterError::MissingField {
                platform: platform.key(),
                field: "dbMajor",
            })?,
            major_location: self.major_location.ok_or(RosterError::MissingField {
                platform: platform.key(),
                field: "dbMajorLoc",
            })?,
            fut_version: self.fut_version,
            fut_location: self.fut_location,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<rosterUpdate><squadinfoset>
        <squadInfo platform="ps4">
            <dbMajor>464</dbMajor>
            <dbMajorLoc>fc/fclive/squads/464/Squads20260218000000</dbMajorLoc>
            <dbFUTVer>464.fut</dbFUTVer>
            <dbFUTLoc>fc/fclive/futsquads/464/FutSquads</dbFUTLoc>
        </squadInfo>
        <squadInfo platform="Xbox">
            <dbMajor>464</dbMajor>
            <dbMajorLoc>fc/fclive/squads/464/xbox_squads</dbMajorLoc>
        </squadInfo>
        <squadInfo platform="PC">
            <dbMajor>1</dbMajor>
            <dbMajorLoc>pc</dbMajorLoc>
        </squadInfo>
    </squadinfoset></rosterUpdate>"#;

    #[test]
    fn parses_ps4_and_xbox_entries_and_ignores_other_platforms() {
        let manifests = parse_manifests(SAMPLE).expect("valid manifest");
        assert_eq!(manifests.len(), 2);
        let ps4 = find(&manifests, Platform::Ps4).expect("ps4 entry");
        assert_eq!(ps4.major_version, "464");
        assert_eq!(
            ps4.major_location,
            "fc/fclive/squads/464/Squads20260218000000"
        );
        assert_eq!(ps4.fut_version.as_deref(), Some("464.fut"));
        let xbox = find(&manifests, Platform::Xbox).expect("xbox entry");
        assert_eq!(xbox.major_location, "fc/fclive/squads/464/xbox_squads");
        assert!(xbox.fut_location.is_none());
    }

    #[test]
    fn element_names_are_case_insensitive() {
        let xml = r#"<r><SquadInfo Platform="ps4">
            <DBMAJOR>464</DBMAJOR>
            <dbmajorloc>fc/fclive/squads/464/x</dbmajorloc>
        </SquadInfo></r>"#;
        let manifests = parse_manifests(xml).expect("valid manifest");
        let ps4 = find(&manifests, Platform::Ps4).expect("ps4 entry");
        assert_eq!(ps4.major_version, "464");
        assert_eq!(ps4.major_location, "fc/fclive/squads/464/x");
    }

    #[test]
    fn reports_missing_required_field() {
        let xml = r#"<r><squadinfoset>
            <squadInfo platform="ps4"><dbMajor>464</dbMajor></squadInfo>
        </squadinfoset></r>"#;
        assert!(matches!(
            parse_manifests(xml),
            Err(RosterError::MissingField {
                field: "dbMajorLoc",
                ..
            })
        ));
    }
}
