
#[derive(Debug)]
pub struct OsuMapEvents {
    pub bg_filename: Option<String>,
}

impl OsuMapEvents {
    pub fn new(map: &str) -> Self {
        let mut bg_filename: Option<String> = None;

        for (i, line) in map.lines().enumerate() {
            if i == 2 {
                let bg_settings: Vec<&str> = line.split(',').collect();
                if bg_settings.len() > 2 {
                    let mut raw_bg_filepath = bg_settings[2].chars();
                    raw_bg_filepath.next();
                    raw_bg_filepath.next_back();
                    bg_filename = Some(raw_bg_filepath.as_str().to_string());
                }
                break;
            }
        }

        Self { bg_filename }
    }
}
