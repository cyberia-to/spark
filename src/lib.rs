//! Spark — the opener for a family of files.
//!
//! A spark draws `File` data. It is not a format (jpg is a format; `image`
//! is the spark). It is not a particle. Not every particle has a spark:
//! a key, a neuron, an unknown blob stay on the particle page.
//!
//! Resolve order: sniffed kind (axons of type come next, in the host that
//! can see the graph). Cyb glides `://particle/` → `://file/` only when
//! [`resolve`] returns `Some`.

use particle::{File, Kind};

/// Named family of openers that ship with cyb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SparkId {
    Text,
    Image,
}

/// What a spark hands spacetime. Bevy-free: the shell mounts this.
#[derive(Clone, Debug)]
pub enum Surface {
    Text(String),
    /// Raw encoded image; the shell (or a later decoder) paints pixels.
    Image {
        kind: Kind,
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug)]
pub struct SparkError {
    pub message: String,
}

/// Kind → spark. No axon table here — the graph host passes a hint later.
pub fn resolve(file: &File) -> Option<SparkId> {
    match file.kind() {
        Kind::Text => Some(SparkId::Text),
        Kind::ImagePng | Kind::ImageJpeg | Kind::ImageGif | Kind::ImageWebp => Some(SparkId::Image),
        Kind::Opaque => None,
    }
}

/// Open with the resolved spark. `None` from [`resolve`] is not an error
/// — it means stay on the particle page.
pub fn open(file: &File) -> Result<Option<Surface>, SparkError> {
    let Some(id) = resolve(file) else {
        return Ok(None);
    };
    match id {
        SparkId::Text => open_text(file).map(Some),
        SparkId::Image => Ok(Some(Surface::Image {
            kind: file.kind(),
            bytes: file.data.clone(),
        })),
    }
}

fn open_text(file: &File) -> Result<Surface, SparkError> {
    match core::str::from_utf8(&file.data) {
        Ok(s) => Ok(Surface::Text(s.to_string())),
        Err(_) => Err(SparkError {
            message: "text spark: bytes are not utf-8".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use particle::File;

    #[test]
    fn text_resolves_and_opens() {
        let f = File::from_data(b"hello particle".to_vec());
        assert_eq!(resolve(&f), Some(SparkId::Text));
        match open(&f).unwrap() {
            Some(Surface::Text(s)) => assert_eq!(s, "hello particle"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn opaque_has_no_spark() {
        let f = File::from_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 0xff, 0xfe]);
        assert_eq!(resolve(&f), None);
        assert!(open(&f).unwrap().is_none());
    }

    #[test]
    fn png_is_image_spark() {
        let mut data = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        data.extend_from_slice(&[0; 8]);
        let f = File::from_data(data);
        assert_eq!(resolve(&f), Some(SparkId::Image));
    }
}
