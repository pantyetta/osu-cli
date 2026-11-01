#[derive(Debug)]
pub struct OsuMapMetadata {
    pub title: String,
    pub title_unicode: Option<String>,
    pub artist: String,
    pub artist_unicode: Option<String>,
    pub creator: String,
    pub tags: Vec<String>,
}

impl OsuMapMetadata {
    pub fn new(map: &str) -> Self {
        let mut title = String::new();
        let mut title_unicode: Option<String> = None;
        let mut artist = String::new();
        let mut artist_unicode: Option<String> = None;
        let mut creator = String::new();
        let mut tags: Vec<String> = vec![];

        for line in map.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };

            match key {
                "Title" => title = value.trim().to_string(),
                "TitleUnicode" => title_unicode = non_empty(value),
                "Artist" => artist = value.trim().to_string(),
                "ArtistUnicode" => artist_unicode = non_empty(value),
                "Creator" => creator = value.trim().to_string(),
                "Tags" => tags = value.split_whitespace().map(String::from).collect(),
                _ => continue,
            }
        }

        Self {
            title,
            title_unicode,
            artist,
            artist_unicode,
            creator,
            tags,
        }
    }

    pub fn display_title(&self) -> &str {
        self.title_unicode.as_deref().unwrap_or(&self.title)
    }

    pub fn display_artist(&self) -> &str {
        self.artist_unicode.as_deref().unwrap_or(&self.artist)
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}
