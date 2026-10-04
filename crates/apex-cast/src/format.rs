//! Cast container: a 16-byte header followed by a tree of nodes, each with
//! typed, named properties (spec: <https://github.com/dtzxporter/cast>).

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum CastError {
    NotCast,
    Truncated(usize),
    BadPropertyType(u16),
    BadString(usize),
    Missing(&'static str),
}

impl fmt::Display for CastError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCast => write!(f, "not a cast file"),
            Self::Truncated(at) => write!(f, "file ends early (at byte {at})"),
            Self::BadPropertyType(t) => write!(f, "unknown property type {t:#06x}"),
            Self::BadString(at) => write!(f, "unterminated string at byte {at}"),
            Self::Missing(what) => write!(f, "missing {what}"),
        }
    }
}

impl std::error::Error for CastError {}

/// Property payload. Integer arrays keep their stored width.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    V2(Vec<[f32; 2]>),
    V3(Vec<[f32; 3]>),
    V4(Vec<[f32; 4]>),
}

impl Value {
    pub fn len(&self) -> usize {
        match self {
            Self::Str(_) => 1,
            Self::U8(v) => v.len(),
            Self::U16(v) => v.len(),
            Self::U32(v) => v.len(),
            Self::U64(v) => v.len(),
            Self::F32(v) => v.len(),
            Self::F64(v) => v.len(),
            Self::V2(v) => v.len(),
            Self::V3(v) => v.len(),
            Self::V4(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Any integer array widened to u32 (u64 values are truncated).
    pub fn as_u32s(&self) -> Option<Vec<u32>> {
        Some(match self {
            Self::U8(v) => v.iter().map(|&x| u32::from(x)).collect(),
            Self::U16(v) => v.iter().map(|&x| u32::from(x)).collect(),
            Self::U32(v) => v.clone(),
            Self::U64(v) => v.iter().map(|&x| x as u32).collect(),
            _ => return None,
        })
    }

    pub fn first_u64(&self) -> Option<u64> {
        match self {
            Self::U8(v) => v.first().map(|&x| u64::from(x)),
            Self::U16(v) => v.first().map(|&x| u64::from(x)),
            Self::U32(v) => v.first().map(|&x| u64::from(x)),
            Self::U64(v) => v.first().copied(),
            _ => None,
        }
    }

    pub fn first_f32(&self) -> Option<f32> {
        match self {
            Self::F32(v) => v.first().copied(),
            Self::F64(v) => v.first().map(|&x| x as f32),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Node type, e.g. `*b"modl"`.
    pub kind: [u8; 4],
    pub hash: u64,
    pub props: Vec<(String, Value)>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.props.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.prop(name).and_then(Value::as_str)
    }

    pub fn children_of<'a>(&'a self, kind: &'a [u8; 4]) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| &c.kind == kind)
    }

    pub fn child(&self, kind: &[u8; 4]) -> Option<&Node> {
        self.children.iter().find(|c| &c.kind == kind)
    }

    /// Depth-first search for the first node of `kind` (this node included).
    pub fn find(&self, kind: &[u8; 4]) -> Option<&Node> {
        if &self.kind == kind {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(kind))
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], CastError> {
        let end = self
            .at
            .checked_add(n)
            .ok_or(CastError::Truncated(self.at))?;
        let s = self
            .buf
            .get(self.at..end)
            .ok_or(CastError::Truncated(self.at))?;
        self.at = end;
        Ok(s)
    }

    fn u16(&mut self) -> Result<u16, CastError> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, CastError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, CastError> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    fn array<T, const W: usize>(
        &mut self,
        count: usize,
        f: impl Fn([u8; W]) -> T,
    ) -> Result<Vec<T>, CastError> {
        let raw = self.bytes(count.checked_mul(W).ok_or(CastError::Truncated(self.at))?)?;
        Ok(raw
            .chunks_exact(W)
            .map(|c| f(c.try_into().unwrap()))
            .collect())
    }

    fn value(&mut self, ty: u16, count: usize) -> Result<Value, CastError> {
        let f = |b: [u8; 4]| f32::from_le_bytes(b);
        let vec = |b: &[u8], i: usize| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
        Ok(match &ty.to_le_bytes() {
            b"b\0" => Value::U8(self.bytes(count)?.to_vec()),
            b"h\0" => Value::U16(self.array(count, u16::from_le_bytes)?),
            b"i\0" => Value::U32(self.array(count, u32::from_le_bytes)?),
            b"l\0" => Value::U64(self.array(count, u64::from_le_bytes)?),
            b"f\0" => Value::F32(self.array(count, f)?),
            b"d\0" => Value::F64(self.array(count, f64::from_le_bytes)?),
            b"2v" => Value::V2(self.array(count, |b: [u8; 8]| [vec(&b, 0), vec(&b, 1)])?),
            b"3v" => {
                Value::V3(self.array(count, |b: [u8; 12]| [vec(&b, 0), vec(&b, 1), vec(&b, 2)])?)
            }
            b"4v" => Value::V4(self.array(count, |b: [u8; 16]| {
                [vec(&b, 0), vec(&b, 1), vec(&b, 2), vec(&b, 3)]
            })?),
            b"s\0" => {
                let rest = &self.buf[self.at..];
                let end = rest
                    .iter()
                    .position(|&c| c == 0)
                    .ok_or(CastError::BadString(self.at))?;
                let s = String::from_utf8_lossy(&rest[..end]).into_owned();
                self.at += end + 1;
                Value::Str(s)
            }
            _ => return Err(CastError::BadPropertyType(ty)),
        })
    }

    fn node(&mut self) -> Result<Node, CastError> {
        let start = self.at;
        let kind: [u8; 4] = self.bytes(4)?.try_into().unwrap();
        let size = self.u32()? as usize;
        let hash = self.u64()?;
        let prop_count = self.u32()?;
        let child_count = self.u32()?;
        let mut props = Vec::with_capacity(prop_count as usize);
        for _ in 0..prop_count {
            let ty = self.u16()?;
            let name_len = self.u16()? as usize;
            let count = self.u32()? as usize;
            let name = String::from_utf8_lossy(self.bytes(name_len)?).into_owned();
            props.push((name, self.value(ty, count)?));
        }
        let mut children = Vec::with_capacity(child_count as usize);
        for _ in 0..child_count {
            children.push(self.node()?);
        }
        // Trust the declared size so unknown trailing data is skipped.
        if size >= 24 && start + size <= self.buf.len() {
            self.at = start + size;
        }
        Ok(Node {
            kind,
            hash,
            props,
            children,
        })
    }
}

/// Parse a whole Cast file into its root nodes.
pub fn parse(buf: &[u8]) -> Result<Vec<Node>, CastError> {
    if buf.get(0..4) != Some(b"cast") {
        return Err(CastError::NotCast);
    }
    let mut r = Reader { buf, at: 8 };
    let roots = r.u32()?;
    r.at = 16;
    (0..roots).map(|_| r.node()).collect()
}
