use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    songs_dir: PathBuf,
}

pub fn path() -> io::Result<PathBuf> {
    dirs::config_local_dir()
        .map(|dir| dir.join("osu-cli").join("config.toml"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "config directory not found"))
}

pub fn songs_dir(path: &Path) -> io::Result<PathBuf> {
    let text = fs::read_to_string(path).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "failed to read {}: {e} (pass a songs path or set songs_dir in it)",
                path.display()
            ),
        )
    })?;
    let config: Config = toml::from_str(&text).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {}: {e}", path.display()),
        )
    })?;
    Ok(config.songs_dir)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::songs_dir;

    fn write_config(name: &str, text: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("osu-cli-test-{}-{name}.toml", std::process::id()));
        fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn missing_config_is_error() {
        assert!(songs_dir(Path::new("no/such/config.toml")).is_err());
    }

    #[test]
    fn reads_songs_dir() {
        let path = write_config("valid", r"songs_dir = 'D:\osu!\Songs'");
        let dir = songs_dir(&path);
        fs::remove_file(&path).unwrap();

        assert_eq!(dir.unwrap(), Path::new(r"D:\osu!\Songs"));
    }

    #[test]
    fn missing_key_is_error() {
        let path = write_config("no-key", "");
        let dir = songs_dir(&path);
        fs::remove_file(&path).unwrap();

        assert!(dir.is_err());
    }
}
