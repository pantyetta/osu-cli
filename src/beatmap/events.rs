
#[derive(Debug)]
pub struct OsuMapEvents {
    pub bg_filename: Option<String>,
}

impl OsuMapEvents {
    pub fn new(map: &str) -> Self {
        let bg_filename = map.lines().filter(|line| !line.starts_with("//")).find_map(|line| {
            let mut fields = line.split(',');
            let (kind, _, name) = (fields.next()?, fields. next()?, fields.next()?);
            (kind.trim() == "0").then(|| name.trim().trim_matches('"').to_string())
        });
        
        Self { bg_filename }
    }
}
