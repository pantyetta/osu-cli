use std::{path::{Path, PathBuf}, sync::mpsc::{self, Receiver, Sender}, thread};

use image::ImageReader;
use ratatui::layout::Size;
use ratatui_image::{FilterType, Resize, picker::Picker, protocol::Protocol};

pub const ROWS: u16 = 3;

type Loaded = (usize, Option<Protocol>);

pub struct Thumbnail {
    picker: Picker,
    song: Option<usize>,
    pub image: Option<Protocol>,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
}

impl Thumbnail {
    pub fn new(picker: Picker) -> Self {
        let (tx, rx) = mpsc::channel();
       
        Self {
            picker,
            song: None,
            image: None,
            tx,
            rx,
        }
    }

    pub fn cols(&self) -> u16 {
        square_cols(&self.picker)
    }

    pub fn request(&mut self, song: usize, path: Option<PathBuf>) {
        if self.song == Some(song) {
            return;
        }

        self.song = Some(song);
        self.image = None;

        let tx = self.tx.clone();
        let picker = self.picker.clone();
        thread::spawn(move || {
            let image = path.as_deref().and_then(|p| load(p, &picker));
            let _ = tx.send((song, image));
        });
    }

    pub fn poll(&mut self) {
        while let Ok((song, image)) = self.rx.try_recv() {
            if self.song == Some(song) {
                self.image = image;
            }
        }
    }
}

fn load(path: &Path, picker: &Picker) -> Option<Protocol> {
    let image = ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;

    let font = picker.font_size();
    let cols = square_cols(picker);
    let image = image.resize_to_fill(cols as u32 * font.width as u32, ROWS as u32 * font.height as u32, FilterType::Triangle);

    picker.new_protocol(image, Size::new(cols, ROWS), Resize::Fit(None)).ok()
}

fn square_cols(picker: &Picker) -> u16 {
    let font = picker.font_size();
    let width = font.width.max(1);
    (ROWS * font.height + width / 2) / width
}