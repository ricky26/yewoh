use crate::string::recognize_string;
use bevy::prelude::*;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until, take_while1};
use nom::character::complete::{alpha1, alphanumeric1, char, one_of};
use nom::combinator::{map, opt, recognize};
use nom::error::{ErrorKind, ParseError};
use nom::multi::{many0, many0_count, many1};
use nom::sequence::{delimited, pair, tuple};
use nom::{Finish, IResult, InputLength, Parser};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter, Write};
use anyhow::anyhow;

#[derive(Clone)]
pub struct Path<'a>(pub SmallVec<[&'a str; 8]>);

impl<'a> Path<'a> {
    pub fn single(segment: &'a str) -> Path<'a> {
        let mut segments = SmallVec::new();
        segments.push(segment);
        Path(segments)
    }

    pub fn from_iter(parts: impl IntoIterator<Item=&'a str>) -> Path<'a> {
        Path(SmallVec::from_iter(parts))
    }
}

impl<'a> Debug for Path<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut parts = self.0.iter();
        let Some(first) = parts.next() else { return Ok(()); };
        write!(f, "{first}")?;

        for part in parts {
            write!(f, "::{part}")?;
        }

        Ok(())
    }
}

#[derive(Clone)]
pub enum Import<'a> {
    Path(Path<'a>),
    File(&'a str),
}

impl<'a> Debug for Import<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Import::Path(path) => path.fmt(f),
            Import::File(path) => write!(f, "import \"{path}\""),
        }
    }
}

#[derive(Clone)]
pub enum Expression<'a> {
    Number(&'a str),
    String(&'a str),
    Tuple(Option<Path<'a>>, SmallVec<[usize; 8]>),
    Struct(Option<Path<'a>>, SmallVec<[(&'a str, usize); 8]>),
    Path(Path<'a>),
    Import(Import<'a>),
}

impl<'a> Debug for Expression<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Expression::Number(s) => write!(f, "{s}"),
            Expression::String(s) => write!(f, "{s}"),
            Expression::Tuple(name, parts) => {
                if let Some(name) = name {
                    write!(f, "{name:?}(")?;
                } else {
                    f.write_char('(')?;
                }

                let mut parts = parts.iter();
                if let Some(part) = parts.next() {
                    write!(f, "%{part}")?;
                    for part in parts {
                        write!(f, ", %{part}")?;
                    }
                }

                f.write_char(')')
            }
            Expression::Struct(name, parts) => {
                if let Some(name) = name {
                    write!(f, "{name:?}{{")?;
                } else {
                    f.write_char('{')?;
                }

                let mut parts = parts.iter();
                if let Some((key, part)) = parts.next() {
                    write!(f, "{key}: %{part}")?;
                    for (key, part) in parts {
                        write!(f, ", {key}: %{part}")?;
                    }
                }

                f.write_char('}')
            }
            Expression::Path(path) => path.fmt(f),
            Expression::Import(import) => import.fmt(f),
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Local,
    In,
    Out,
}

impl Visibility {
    pub fn as_keyword(self) -> &'static str {
        match self {
            Visibility::In => "in",
            Visibility::Out => "out",
            Visibility::Local => "local",
        }
    }
}

impl Debug for Visibility {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_keyword())
    }
}

#[derive(Clone, Default)]
pub struct Register<'a> {
    pub name: Option<&'a str>,
    pub visibility: Visibility,
    pub variable_type: Option<Path<'a>>,
    pub optional: bool,
    pub value: Option<Expression<'a>>,
}

impl<'a> Register<'a> {
    fn fmt_with_index(&self, index: usize, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{index}")?;

        if let Some(name) = &self.name {
            write!(f, " {:?} {}", self.visibility, name)?;
        }

        if let Some(ty) = &self.variable_type {
            let opt = if self.optional { "?" } else { "" };
            write!(f, ": {ty:?}{opt}")?;
        }

        if let Some(expr) = &self.value {
            write!(f, " = {expr:?}")?;
        }

        Ok(())
    }
}

impl<'a> Debug for Register<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.fmt_with_index(0, f)
    }
}

impl<'a> From<Expression<'a>> for Register<'a> {
    fn from(value: Expression<'a>) -> Self {
        Register {
            name: None,
            visibility: Visibility::Local,
            variable_type: None,
            optional: false,
            value: Some(value),
        }
    }
}

#[derive(Clone)]
pub struct Application {
    pub entity: usize,
    pub expression: usize,
}

impl Debug for Application {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{} <- %{}", self.entity, self.expression)
    }
}

#[derive(Clone, Default)]
pub struct Document<'a> {
    pub registers: Vec<Register<'a>>,
    pub applications: Vec<Application>,
}

impl<'a> Document<'a> {
    pub fn push_register(&mut self, register: impl Into<Register<'a>>) -> usize {
        let index = self.registers.len();
        self.registers.push(register.into());
        index
    }

    pub fn parse(input: &'a str) -> anyhow::Result<Document<'a>> {
        parse_document::<nom::error::Error<&'a str>>(input)
            .map_err(|e| e.to_owned())
            .finish()
            .map_err(|e| e.into())
            .and_then(|(rest, doc)| {
                if rest.is_empty() {
                    Ok(doc)
                } else {
                    Err(anyhow!("unexpected data after document: {rest}"))
                }
            })
    }
}

impl<'a> Debug for Document<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Document {{")?;

        for (index, expr) in self.registers.iter().enumerate() {
            write!(f, "  ")?;
            expr.fmt_with_index(index, f)?;
            writeln!(f, ";")?;
        }

        for application in &self.applications {
            writeln!(f, "  {application:?};")?;
        }

        f.write_char('}')
    }
}

fn fold_separated0<I, O1, O2, O3, E, F1, F2, F3, F4>(
    mut f: F1,
    mut sep: F2,
    mut init: F3,
    mut fold: F4,
) -> impl FnMut(I) -> IResult<I, O3, E>
where
    I: Clone + InputLength,
    E: ParseError<I>,
    F1: Parser<I, O1, E>,
    F2: Parser<I, O2, E>,
    F3: FnMut() -> O3,
    F4: FnMut(O3, O1) -> O3,
{
    move |mut i: I| {
        let mut res = init();

        match f.parse(i.clone()) {
            Err(nom::Err::Error(_)) => return Ok((i, res)),
            Err(e) => return Err(e),
            Ok((i1, o)) => {
                res = fold(res, o);
                i = i1;
            }
        }

        loop {
            let len = i.input_len();
            match sep.parse(i.clone()) {
                Err(nom::Err::Error(_)) => return Ok((i, res)),
                Err(e) => return Err(e),
                Ok((i1, _)) => {
                    if i1.input_len() == len {
                        return Err(nom::Err::Error(E::from_error_kind(i1, ErrorKind::SeparatedList)));
                    }

                    match f.parse(i1.clone()) {
                        Err(nom::Err::Error(_)) => return Ok((i, res)),
                        Err(e) => return Err(e),
                        Ok((i2, o)) => {
                            res = fold(res, o);
                            i = i2;
                        }
                    }
                }
            }
        }
    }
}

fn comment_line<'a, E: ParseError<&'a str>>(src: &'a str) -> IResult<&'a str, &'a str, E> {
    recognize(tuple((
        tag("//"),
        take_until("\n"),
        tag("\n"),
    )))(src)
}

fn comment_multiline<'a, E: ParseError<&'a str>>(src: &'a str) -> IResult<&'a str, &'a str, E> {
    recognize(tuple((
        tag("/*"),
        take_until("*/"),
        tag("*/"),
    )))(src)
}

fn whitespace<'a, E: ParseError<&'a str>>(src: &'a str) -> IResult<&'a str, &'a str, E> {
    alt((
        take_while1(|b: char| b.is_ascii_whitespace()),
        comment_line,
        comment_multiline,
    ))(src)
}

fn whitespace0<'a, E: ParseError<&'a str>>(src: &'a str) -> IResult<&'a str, &'a str, E> {
    recognize(many0_count(whitespace))(src)
}

fn parse_number<'a, E: ParseError<&'a str>>(src: &'a str) -> IResult<&'a str, &'a str, E> {
    fn decimal<'a, E: ParseError<&'a str>>(src: &'a str) -> IResult<&'a str, &'a str, E> {
        recognize(one_of("0123456789_"))(src)
    }

    recognize(tuple((
        alt((
            recognize(tuple((
                char('.'),
                many1(decimal),
            ))),
            recognize(tuple((
                many1(decimal),
                char('.'),
                many0(decimal),
            ))),
        )),
        opt(tuple((
            one_of("eE"),
            opt(one_of("+-")),
            many1(decimal),
        ))),
    )))(src)
}

fn parse_identifier<'a, E: ParseError<&'a str>>(src: &'a str) -> IResult<&'a str, &'a str, E> {
    recognize(
        pair(
            alt((alpha1, tag("_"), tag("$"))),
            many0_count(alt((alphanumeric1, tag("_"), tag("$")))),
        ),
    )(src)
}

fn parse_tuple_body<'a, 'd: 'a, E: ParseError<&'d str> + 'a>(
    document: &'a RefCell<Document<'d>>,
) -> impl FnMut(&'d str) -> IResult<&'d str, SmallVec<[usize; 8]>, E> + 'a {
    delimited(
        tag("("),
        fold_separated0(
            parse_expression(document),
            delimited(whitespace0, tag(","), whitespace0),
            || SmallVec::new(),
            |mut acc, v| {
                acc.push(v);
                acc
            },
        ),
        tag(")"),
    )
}

fn parse_struct_body<'a, 'd: 'a, E: ParseError<&'d str>>(
    _document: &'a RefCell<Document<'d>>,
) -> impl FnMut(&'d str) -> IResult<&'d str, SmallVec<[(&'d str, usize); 8]>, E> + 'a {
    |_| todo!()
}

fn parse_path<'a, E: ParseError<&'a str>>(src: &'a str) -> IResult<&'a str, Path, E> {
    let (mut rest, start) = parse_identifier(src)?;
    let mut result = SmallVec::new();
    result.push(start);

    let mut delimiter = delimited(whitespace0, tag("::"), whitespace0);
    loop {
        let next = match delimiter(rest) {
            Ok((next, _)) => next,
            Err(nom::Err::Error(_)) => return Ok((rest, Path(result))),
            Err(e) => return Err(e),
        };

        match parse_identifier(next) {
            Ok((next, ident)) => {
                rest = next;
                result.push(ident);
            }
            Err(nom::Err::Error(_)) => return Ok((rest, Path(result))),
            Err(e) => return Err(e),
        }
    }
}

fn parse_path_import<'a, 'd: 'a, E: ParseError<&'d str>>(
    _document: &'a RefCell<Document<'d>>,
) -> impl FnMut(&'d str) -> IResult<&'d str, usize, E> + 'a {
    |_| todo!()
}

fn parse_file_import<'a, 'd: 'a, E: ParseError<&'d str> + 'a>(
    document: &'a RefCell<Document<'d>>,
) -> impl FnMut(&'d str) -> IResult<&'d str, usize, E> + 'a {
    map(
        tuple((
            recognize_string,
            delimited(whitespace0, tag("as"), whitespace0),
            parse_identifier,
        )),
        |(path, _, ident)| {
            document.borrow_mut().push_register(Register {
                name: ident.into(),
                value: Some(Expression::Import(Import::File(path))),
                ..default()
            })
        },
    )
}

fn parse_import<'a, 'd: 'a, E: ParseError<&'d str> + 'a>(
    document: &'a RefCell<Document<'d>>,
) -> impl FnMut(&'d str) -> IResult<&'d str, usize, E> + 'a {
    map(
        tuple((
            tag("import"),
            whitespace0,
            alt((
                parse_path_import(document),
                parse_file_import(document),
            )),
        )),
        |(_, _, x)| x,
    )
}

fn parse_expression<'a, 'd: 'a, E: ParseError<&'d str> + 'a>(
    document: &'a RefCell<Document<'d>>,
) -> impl FnMut(&'d str) -> IResult<&'d str, usize, E> + 'a {
    alt((
        map(parse_number, |src| document.borrow_mut().push_register(Expression::Number(src))),
        map(recognize_string, |src| document.borrow_mut().push_register(Expression::String(src))),
        parse_import(document),
        map(
            pair(
                parse_path,
                alt((
                    map(parse_tuple_body(document), |v| Expression::Tuple(None, v)),
                    map(parse_struct_body(document), |v| Expression::Struct(None, v)),
                )),
            ),
            |(ident, expr)| {
                let expr = match expr {
                    Expression::Tuple(_, body) => Expression::Tuple(Some(ident), body),
                    Expression::Struct(_, body) => Expression::Struct(Some(ident), body),
                    _ => unreachable!(),
                };
                document.borrow_mut().push_register(expr)
            },
        ),
    ))
}

fn parse_document<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Document<'a>, E> {
    let document: RefCell<Document<'a>> = RefCell::new(Document::default());
    let (rest, _) = many0(alt((
        delimited(whitespace0, parse_expression(&document), whitespace0),
    )))(input)?;
    let document: Document<'a> = document.into_inner();
    Ok((rest, document))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test() {
        let doc = Document {
            registers: vec![
                Register {
                    name: Some("var"),
                    visibility: Visibility::In,
                    variable_type: Some(Path::from_iter(["bevy", "Transform"])),
                    optional: true,
                    value: Some(Expression::Import(Import::Path(Path::single("test2")))),
                },
                Expression::Import(Import::Path(Path::single("test"))).into(),
                Expression::Import(Import::File("myfile.fab")).into(),
                Expression::Tuple(None, SmallVec::from_iter([0, 1, 2])).into(),
                Expression::Tuple(Some(Path::single("MyTuple")), SmallVec::from_iter([0, 1, 2])).into(),
                Expression::Struct(None, SmallVec::from_iter([("field1", 0), ("field2", 1)])).into(),
                Expression::Struct(Some(Path::single("MyStruct")), SmallVec::from_iter([("field1", 0), ("field2", 1)])).into(),
            ],
            applications: vec![
                Application { entity: 0, expression: 0 },
            ],
        };

        println!("{doc:?}");
    }

    #[test]
    fn test_parse() {
        let doc = Document::parse("
            in param1: f32 = 5.0;
            in param2: f32? = 0.4;
            out result0: f32;
            local myLocal_: Entity?;
            local test = MyTuple(1, 2, 3.4);
            $ <- MyComponent {
                field1: 1.0,
                field2: myLocal_,
            };
        ").unwrap();
        println!("{doc:?}");
    }
}
