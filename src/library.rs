use std::{
    io,
    path::{Path, PathBuf},
};

use indicatif::ParallelProgressIterator;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::beatmap::OsuMap;

pub fn load(root: &Path) -> io::Result<Vec<OsuMap>> {
    let dirs = entries(root, Path::is_dir)?;
    Ok(dirs
        .par_iter()
        .progress_count(dirs.len() as u64)
        .flat_map_iter(|dir| load_maps_in_dir(dir))
        .collect())
}

fn entries(path: &Path, keep: fn(&Path) -> bool) -> io::Result<Vec<PathBuf>> {
    Ok(path
        .read_dir()?
        .filter_map(|item| item.ok())
        .map(|item| item.path())
        .filter(|path| keep(path))
        .collect())
}

fn load_maps_in_dir(dir: &Path) -> Vec<OsuMap> {
    let mut maps: Vec<OsuMap> = Vec::new();
    for file in entries(dir, Path::is_file).unwrap_or_default() {
        if let Ok(map) = OsuMap::from_file(&file) {
            let is_dup = maps.iter().any(|m| {
                m.metadata.title == map.metadata.title
                    && m.general.audio_filename == map.general.audio_filename
            });
            if !is_dup {
                maps.push(map);
            }
        }
    }
    maps
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::load;

    const MAP: &str = "osu file format v14\r\n\r\n\
        [General]\r\nAudioFilename: audio.mp3\r\n\r\n\
        [Metadata]\r\nTitle:Song\r\n\r\n\
        [Events]\r\n";

    #[test]
    fn missing_root_is_error() {
        assert!(load(Path::new("no/such/songs")).is_err());
    }

    #[test]
    fn skips_broken_files_and_duplicates() {
        let root = std::env::temp_dir().join(format!("osu-cli-test-{}", std::process::id()));
        let dir = root.join("song");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("easy.osu"), MAP).unwrap();
        fs::write(dir.join("hard.osu"), MAP).unwrap();
        fs::write(dir.join("broken.osu"), [0xff, 0xfe]).unwrap();
        fs::write(dir.join("audio.mp3"), []).unwrap();

        let maps = load(&root);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(maps.unwrap().len(), 1);
    }
}
