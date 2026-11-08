use std::collections::BTreeMap;
#[derive(Clone, PartialEq, Eq)]
pub enum Content {
    Text(String),
    Png(Vec<u8>),
}
impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(t) => f.debug_struct("Text").field("bytes", &t.len()).finish(),
            Self::Png(p) => f.debug_struct("Png").field("bytes", &p.len()).finish(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub origin: [u8; 16],
    pub generation: u64,
    pub content: Content,
}
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid clipboard update")]
pub struct ClipboardError;
pub struct ClipboardSync {
    origin: [u8; 16],
    generation: u64,
    seen: BTreeMap<[u8; 16], u64>,
    last: Option<[u8; 32]>,
}
impl ClipboardSync {
    pub fn new(origin: [u8; 16]) -> Self {
        Self {
            origin,
            generation: 0,
            seen: BTreeMap::new(),
            last: None,
        }
    }
    pub fn local(&mut self, content: Content) -> Result<Option<Update>, ClipboardError> {
        let digest = validate(&content)?;
        if self.last == Some(digest) {
            return Ok(None);
        }
        self.generation = self.generation.checked_add(1).ok_or(ClipboardError)?;
        self.last = Some(digest);
        Ok(Some(Update {
            origin: self.origin,
            generation: self.generation,
            content,
        }))
    }
    pub fn receive(&mut self, update: Update) -> Result<Option<Content>, ClipboardError> {
        if update.generation == 0 {
            return Err(ClipboardError);
        }
        let digest = validate(&update.content)?;
        if update.origin == self.origin
            || self
                .seen
                .get(&update.origin)
                .is_some_and(|g| update.generation <= *g)
        {
            return Ok(None);
        }
        if self.seen.len() >= 64 && !self.seen.contains_key(&update.origin) {
            return Err(ClipboardError);
        }
        self.seen.insert(update.origin, update.generation);
        if self.last == Some(digest) {
            return Ok(None);
        }
        self.last = Some(digest);
        Ok(Some(update.content))
    }
}
fn validate(content: &Content) -> Result<[u8; 32], ClipboardError> {
    let mut hash = blake3::Hasher::new();
    match content {
        Content::Text(text) => {
            if text.len() > 1024 * 1024 || text.contains('\0') {
                return Err(ClipboardError);
            }
            hash.update(b"deskharbor clipboard text\0");
            hash.update(text.as_bytes());
        }
        Content::Png(bytes) => {
            if bytes.len() > 1024 * 1024 {
                return Err(ClipboardError);
            }
            let mut decoder = png::Decoder::new_with_limits(
                std::io::Cursor::new(bytes),
                png::Limits {
                    bytes: 16 * 1024 * 1024,
                },
            );
            decoder.set_ignore_text_chunk(true);
            decoder.set_ignore_iccp_chunk(true);
            let mut reader = decoder.read_info().map_err(|_| ClipboardError)?;
            if reader.info().animation_control.is_some() {
                return Err(ClipboardError);
            }
            let size = reader
                .output_buffer_size()
                .filter(|n| *n <= 16 * 1024 * 1024)
                .ok_or(ClipboardError)?;
            let mut pixels = Vec::new();
            pixels.try_reserve_exact(size).map_err(|_| ClipboardError)?;
            pixels.resize(size, 0);
            reader.next_frame(&mut pixels).map_err(|_| ClipboardError)?;
            reader.finish().map_err(|_| ClipboardError)?;
            hash.update(b"deskharbor clipboard png\0");
            hash.update(bytes);
        }
    }
    Ok(*hash.finalize().as_bytes())
}
