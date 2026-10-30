use std::{error::Error, fs::File, io::Read, path::{Path, PathBuf}};

mod general;
mod metadata;
mod events;

use general::OsuMapGeneral;
use metadata::OsuMapMetadata;
use events::OsuMapEvents;

#[derive(Debug)]
pub struct OsuMap {
    dir: PathBuf,
    pub general: OsuMapGeneral,
    pub metadata: OsuMapMetadata,
    pub events: OsuMapEvents,
}

impl OsuMap {
    pub fn new(map: &str, dir: PathBuf) -> Result<Self, Box<dyn Error>> {
        let sections = |name: &str| {
            map.split("\r\n\r\n")
                .find(|s| s.lines().next() == Some(name))
                .ok_or(format!("Failed to load {} section.", name))
        };

        Ok(Self {
            dir,
            general: OsuMapGeneral::new(sections("[General]")?),
            metadata: OsuMapMetadata::new(sections("[Metadata]")?),
            events: OsuMapEvents::new(sections("[Events]")?),
        })
    }

    pub fn from_file(path: &Path) -> Result<OsuMap, Box<dyn Error>> {
        if path.is_dir() || path.is_empty() {
            return Err(format!("{} is not files.", path.display()).into());
        }

        let is_mapfile = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext_str| ext_str == "osu")
            .unwrap_or(false);
        if !is_mapfile {
            return Err(format!("{} is not osu files.", path.display()).into());
        }

        let mut file = File::open(path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        let dir = path
            .parent()
            .ok_or_else(|| format!("{} has no parent dir.", path.display()))?
            .to_path_buf();

        Self::new(&contents, dir)
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
