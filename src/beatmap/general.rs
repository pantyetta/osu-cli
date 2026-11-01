#[derive(Debug)]
pub struct OsuMapGeneral {
    pub audio_filename: String,
}

impl OsuMapGeneral {
    pub fn new(map: &str) -> Result<Self, String> {
        let audio_filename = map
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                (key == "AudioFilename").then(|| value.trim().to_string())
            })
            .filter(|name| !name.is_empty())
            .ok_or("Failed to load AudioFilename.")?;

        Ok(Self { audio_filename })
    }
}
