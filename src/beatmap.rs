use md5::{Digest, Md5};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

mod events;
mod general;
mod metadata;

use events::OsuMapEvents;
use general::OsuMapGeneral;
use metadata::OsuMapMetadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct OsuMap {
    dir: PathBuf,
    pub hashes: Vec<[u8; 16]>,
    pub general: OsuMapGeneral,
    pub metadata: OsuMapMetadata,
    pub events: OsuMapEvents,
}

impl OsuMap {
    pub fn new(map: &str, dir: PathBuf) -> Result<Self, Box<dyn Error>> {
        let sections = |name: &str| {
            map.split("\n[")
                .find(|s| {
                    s.lines()
                        .next()
                        .is_some_and(|l| l.trim().trim_matches(['[', ']']) == name)
                })
                .ok_or(format!("Failed to load [{}] section.", name))
        };

        Ok(Self {
            dir,
            hashes: Vec::new(),
            general: OsuMapGeneral::new(sections("General")?)?,
            metadata: OsuMapMetadata::new(sections("Metadata")?),
            events: OsuMapEvents::new(sections("Events")?),
        })
    }

    pub fn from_file(path: &Path) -> Result<OsuMap, Box<dyn Error>> {
        let is_mapfile = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext_str| ext_str == "osu")
            .unwrap_or(false);
        if !is_mapfile {
            return Err(format!("{} is not osu files.", path.display()).into());
        }

        let bytes = fs::read(path)?;
        let text = std::str::from_utf8(&bytes)?;

        let dir = path
            .parent()
            .ok_or_else(|| format!("{} has no parent dir.", path.display()))?
            .to_path_buf();

        let mut map = Self::new(text, dir)?;
        map.hashes.push(Md5::digest(&bytes).into());
        Ok(map)
    }

    pub fn audio_path(&self) -> PathBuf {
        self.dir.join(&self.general.audio_filename)
    }

    pub fn bg_path(&self) -> Option<PathBuf> {
        self.events.bg_filename.as_ref().map(|f| self.dir.join(f))
    }

    pub fn matches(&self, query: &str) -> bool {
        let m = &self.metadata;
        m.title.to_lowercase().contains(query)
            || m.artist.to_lowercase().contains(query)
            || m.creator.to_lowercase().contains(query)
            || m.tags.iter().any(|t| t.to_lowercase().contains(query))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::beatmap::OsuMap;

    const SAMPLE: &str = "osu file format v14\r\n\r\n\
        [General]\r\nAudioFilename: audio.mp3\r\n\r\n\
        [Metadata]\r\nTitle:Song\r\nTitleUnicode:曲\r\nArtist:Someone\r\nCreator:mapper\r\nTags:foo Bar\r\n\r\n\
        [Events]\r\n//Background and Video events\r\n0,0,\"bg.jpg\",0,0\r\n";

    fn sample() -> OsuMap {
        OsuMap::new(SAMPLE, PathBuf::from("songs")).unwrap()
    }

    #[test]
    fn resolves_paths_from_map_dir() {
        let map = sample();
        assert_eq!(map.audio_path(), Path::new("songs").join("audio.mp3"));
        assert_eq!(map.bg_path(), Some(Path::new("songs").join("bg.jpg")));
    }

    #[test]
    fn prefers_unicode_title() {
        assert_eq!(sample().metadata.display_title(), "曲");
    }

    #[test]
    fn matches_tags_case_insensitively() {
        assert!(sample().matches("bar"));
        assert!(!sample().matches("missing"));
    }

    #[test]
    fn parses_lf_line_endings() {
        let map = OsuMap::new(&SAMPLE.replace("\r\n", "\n"), PathBuf::new()).unwrap();
        assert_eq!(map.general.audio_filename, "audio.mp3");
        assert_eq!(map.metadata.title, "Song");
        assert_eq!(map.bg_path(), Some(PathBuf::from("bg.jpg")));
    }

    #[test]
    fn empty_unicode_fields_fall_back() {
        let map = SAMPLE.replace("TitleUnicode:曲", "TitleUnicode:\r\nArtistUnicode: ");
        let map = OsuMap::new(&map, PathBuf::new()).unwrap();
        assert_eq!(map.metadata.display_title(), "Song");
        assert_eq!(map.metadata.display_artist(), "Someone");
    }

    #[test]
    fn missing_audio_filename_is_error() {
        let map = SAMPLE.replace("AudioFilename: audio.mp3", "AudioFilename: ");
        assert!(OsuMap::new(&map, PathBuf::new()).is_err());
    }

    #[test]
    fn reads_fields_after_tags() {
        let map = SAMPLE.replace("Tags:foo Bar", "Tags:foo Bar\r\nCreator:late");
        let map = OsuMap::new(&map, PathBuf::new()).unwrap();
        assert_eq!(map.metadata.creator, "late");
    }

    #[test]
    fn unreadable_file_is_error() {
        assert!(OsuMap::from_file(Path::new("no/such/map.osu")).is_err());
    }

    #[test]
    fn missing_section_is_error() {
        assert!(OsuMap::new("[General]\r\nAudioFilename: a.mp3", PathBuf::new()).is_err());
    }
}
