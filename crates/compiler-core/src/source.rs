#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub source: SourceId,
    pub start: usize,
    pub end: usize,
}
impl Span {
    #[must_use]
    pub const fn new(source: SourceId, start: usize, end: usize) -> Self {
        Self { source, start, end }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceMap {
    sources: Vec<Source>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Source {
    name: String,
    text: String,
}

impl SourceMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> SourceId {
        let id = SourceId(self.sources.len());
        self.sources.push(Source {
            name: name.into(),
            text: text.into(),
        });
        id
    }

    #[must_use]
    pub fn name(&self, id: SourceId) -> Option<&str> {
        self.sources.get(id.0).map(|source| source.name.as_str())
    }

    #[must_use]
    pub fn source(&self, id: SourceId) -> Option<&str> {
        self.sources.get(id.0).map(|source| source.text.as_str())
    }

    #[must_use]
    pub fn line_column(&self, id: SourceId, offset: usize) -> Option<(usize, usize)> {
        let text = self.source(id)?;
        if offset > text.len() || !text.is_char_boundary(offset) {
            return None;
        }

        let prefix = &text[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit_once('\n')
            .map_or(prefix, |(_, tail)| tail)
            .chars()
            .count()
            + 1;
        Some((line, column))
    }
}
