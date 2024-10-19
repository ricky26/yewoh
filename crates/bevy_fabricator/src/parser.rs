use std::fmt::{Debug, Display, Formatter};

pub trait SourcePosition {
    fn address(&self, f: &mut Formatter<'_>) -> std::fmt::Result;
}

impl<P: SourcePosition> SourcePosition for &P {
    fn address(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        (*self).address(f)
    }
}

impl<'a> SourcePosition for &'a str {
    fn address(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let n = self.len().min(15);
        let trunc = self.len() > n;
        let slice = &self[..n];
        let trunc = if trunc { "..." } else { "" };
        write!(f, "near '{slice}{trunc}'")
    }
}

impl SourcePosition for String {
    fn address(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <&str as SourcePosition>::address(&self.as_str(), f)
    }
}

#[derive(Clone, Debug)]
pub struct FilePosition {
    pub file: String,
    pub line: usize,
    pub offset: usize,
}

impl FilePosition {
    pub fn from_file_and_str(
        file: impl Into<String>,
        contents: &str,
        remaining: &str,
    ) -> FilePosition {
        let file = file.into();
        let rlen = remaining.len();
        let clen = contents.len();
        if clen < rlen {
            panic!("FilePosition created with invalid remaining string");
        }

        let used = &contents[..(clen - rlen)];
        let mut line = 1;
        let mut offset = 0;

        for c in used.chars() {
            if c == '\n' {
                line += 1;
                offset = 0;
            } else {
                offset += 1;
            }
        }

        FilePosition {
            file,
            line,
            offset,
        }
    }
}

impl SourcePosition for FilePosition {
    fn address(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", &self.file, self.line, self.offset)
    }
}

pub struct DisplayAddress<P>(pub P);

impl<P: SourcePosition> Display for DisplayAddress<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.address(f)
    }
}

pub struct FormatterFn<F: Fn(&mut Formatter<'_>) -> std::fmt::Result>(pub F);

impl<F: Fn(&mut Formatter<'_>) -> std::fmt::Result> Display for FormatterFn<F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0(f)
    }
}

impl<F: Fn(&mut Formatter<'_>) -> std::fmt::Result> Debug for FormatterFn<F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0(f)
    }
}
