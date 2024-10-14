use std::fmt::{Debug, Display, Formatter};

pub trait SourcePosition : Display + Debug {
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

pub struct DisplayAddress<P>(pub P);

impl<P: SourcePosition> Display for DisplayAddress<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.address(f)
    }
}
