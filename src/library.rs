use std::path::{Path, PathBuf};

use indicatif::ParallelProgressIterator;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::beatmap::OsuMap;

pub fn load(root: &Path) -> Vec<OsuMap> {
    let dirs = load_dir(root);
    dirs.par_iter()
        .progress_count(dirs.len() as u64)
        .flat_map_iter(|dir| load_maps_in_dir(dir))
        .collect()
}


fn load_dir(path: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for item in path.read_dir().unwrap() {
        let item = item.unwrap();
        if item.path().is_dir() {
            dirs.push(item.path());
        }
    }

    dirs
}

fn load_files(path: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    for item in path.read_dir().unwrap() {
        let item = item.unwrap();
        if item.path().is_file() {
            files.push(item.path());
        }
    }

    files
}

fn load_maps_in_dir(dir: &Path) -> Vec<OsuMap> {
    let mut maps: Vec<OsuMap> = Vec::new();
    for file in load_files(dir) {
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
