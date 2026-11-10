use std::{
    collections::BTreeMap,
    fs, io,
    path::{self, Path, PathBuf},
    time::SystemTime,
};

use indicatif::ParallelProgressIterator;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::beatmap::OsuMap;

#[derive(Serialize, Deserialize)]
struct CachedDir {
    modified: SystemTime,
    maps: Vec<OsuMap>,
}

type Cache = BTreeMap<PathBuf, CachedDir>;

#[derive(Serialize, Deserialize)]
struct CacheFile<T> {
    version: u32,
    dirs: T,
}

const CACHE_VERSION: u32 = 1;

pub fn load(root: &Path, cache_path: &Path, rescan: bool) -> io::Result<Vec<OsuMap>> {
    let root = path::absolute(root)?;
    let mut old = read_cache(cache_path);
    if rescan {
        old.retain(|dir, _| !dir.starts_with(&root));
    }

    let mut current = Cache::new();
    let mut misses = Vec::new();
    for item in root.read_dir()?.flatten() {
        let Ok(meta) = item.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }

        let dir = item.path();
        match old.remove(&dir) {
            Some(cached) if cached.modified == modified => {
                current.insert(dir, cached);
            }
            _ => misses.push((dir, modified)),
        }
    }

    let removed = old.keys().any(|dir| dir.starts_with(&root));
    old.retain(|dir, _| !dir.starts_with(&root));
    let changed = rescan || removed || !misses.is_empty();

    let loaded: Vec<_> = misses
        .par_iter()
        .progress_count(misses.len() as u64)
        .map(|(dir, modified)| {
            let maps = load_maps_in_dir(dir);
            (
                dir.clone(),
                CachedDir {
                    modified: *modified,
                    maps,
                },
            )
        })
        .collect();

    current.extend(loaded);

    if changed {
        let all: BTreeMap<_, _> = old.iter().chain(current.iter()).collect();
        write_cache(cache_path, &all);
    }

    Ok(current
        .into_values()
        .flat_map(|cached| cached.maps)
        .collect())
}

fn read_cache(path: &Path) -> Cache {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<CacheFile<Cache>>(&bytes).ok())
        .filter(|file| file.version == CACHE_VERSION)
        .map(|file| file.dirs)
        .unwrap_or_default()
}

fn write_cache(path: &Path, dirs: &impl Serialize) {
    let Some(parent) = path.parent() else { return };
    let file = CacheFile {
        version: CACHE_VERSION,
        dirs,
    };
    let Ok(bytes) = serde_json::to_vec(&file) else {
        return;
    };
    let tmp = path.with_extension("tmp");
    let _ = fs::create_dir_all(parent)
        .and_then(|_| fs::write(&tmp, bytes))
        .and_then(|_| fs::rename(&tmp, path));
}

fn load_maps_in_dir(dir: &Path) -> Vec<OsuMap> {
    let mut maps: Vec<OsuMap> = Vec::new();
    let files = dir
        .read_dir()
        .into_iter()
        .flatten()
        .flatten()
        .map(|item| item.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "osu"));

    for file in files {
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
    
    use super::{load, read_cache, write_cache, Cache, CACHE_VERSION};

    const MAP: &str = "osu file format v14\r\n\r\n\
        [General]\r\nAudioFilename: audio.mp3\r\n\r\n\
        [Metadata]\r\nTitle:Song\r\n\r\n\
        [Events]\r\n";

    #[test]
    fn missing_root_is_error() {
        let cache = std::env::temp_dir().join("osu-cli-test-missing.json");
        assert!(load(Path::new("no/such/songs"), &cache, false).is_err());
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

        let cache = root.join("library.json");
        let maps = load(&root, &cache, false);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(maps.unwrap().len(), 1);
    }

    #[test]
    fn cache_with_other_version_is_discarded() {
        let path = std::env::temp_dir()
            .join(format!("osu-cli-test-cache-{}.json", std::process::id()));

        write_cache(&path, &Cache::new());
        let bytes = fs::read(&path).unwrap();
        let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["version"], CACHE_VERSION);

        json["version"] = (CACHE_VERSION + 1).into();
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        let cache = read_cache(&path);
        fs::remove_file(&path).unwrap();

        assert!(cache.is_empty());
    }
}
