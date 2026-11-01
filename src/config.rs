use std::{
    fs,
    io::{self, BufRead, Write},
    path::{self, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
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

pub fn load_or_setup() -> io::Result<PathBuf> {
    let path = path()?;
    match songs_dir(&path) {
        Ok(dir) if dir.is_dir() => Ok(dir),
        Ok(dir) => {
            println!("Songs foler not found: {}", dir.display());
            println!("Enter the path to your osu! Songs folder");
            setup(&path)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            println!("First run: enter the path to your osu! Songs folder.");
            setup(&path)
        }
        Err(e) => Err(e),
    }
}

fn setup(path: &Path) -> io::Result<PathBuf> {
    let dir = read_songs_dir(io::stdin().lock())?;
    save(&path, &dir)?;
    println!("saved to {}", path.display());
    Ok(dir)
}

fn read_songs_dir(mut input: impl BufRead) -> io::Result<PathBuf> {
    loop {
        print!("Songs folder:");
        io::stdout().flush()?;

        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "no songs folder entered",
            ));
        }

        let line = line.trim().trim_matches('"');
        if line.is_empty() {
            continue;
        }

        let dir = path::absolute(line)?;
        if dir.is_dir() {
            return Ok(dir);
        }
        println!("Not a folder: {}", dir.display());
    }
}

fn save(path: &Path, songs_dir: &Path) -> io::Result<()> {
    let config = Config {
        songs_dir: songs_dir.to_path_buf(),
    };

    let text =
        toml::to_string(&config).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use crate::config::{read_songs_dir, save};

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

    #[test]
    fn saved_config_can_be_read_back() {
        let root = std::env::temp_dir().join(format!("osu-cli-test-save-{}", std::process::id()));
        let path = root.join("nested").join("config.toml");
        save(&path, Path::new(r"D:\osu!\Songs")).unwrap();
        let dir = songs_dir(&path);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(dir.unwrap(), Path::new(r"D:\osu!\Songs"));
    }

    #[test]
    fn prompt_retries_until_folder_exists() {
        let temp = std::env::temp_dir();
        let input = format!("\nno/such/songs\n\"{}\"\n", temp.display());

        let dir = read_songs_dir(input.as_bytes()).unwrap();

        assert_eq!(dir, std::path::absolute(&temp).unwrap());
    }

    #[test]
    fn prompt_without_input_is_error() {
        assert!(read_songs_dir(&b""[..]).is_err());
    }
}
