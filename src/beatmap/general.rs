#[derive(Debug)]
pub struct OsuMapGeneral {
    pub audio_filename: String,
}

impl OsuMapGeneral {
    pub fn new(map: &str) -> Self {
        let mut audio_filename = String::new();

        for line in map.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };

            match key {
                "AudioFilename" => {
                    audio_filename = value.trim().to_string();
                    break;
                }
                _ => continue,
            }
        }

        Self { audio_filename }
    }
}
