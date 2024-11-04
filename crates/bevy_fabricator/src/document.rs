use std::fmt::{Debug, Formatter, Write};
use std::str::FromStr;
use bevy::prelude::*;
use derive_more::{Display, Error, From};
use smallvec::SmallVec;

use crate::parser::{SourcePosition, DisplayAddress, FormatterFn};
use crate::string::{escape_string, parse_string, recognize_string};

fn dot_register_name(index: usize) -> impl Display {
    FormatterFn(move |f| write!(f, "v{index}"))
}

#[derive(Clone, Debug, Error, Display, From)]
pub enum ParseError<P: SourcePosition> {
    #[from]
    StringError(super::string::ParseError<P>),
    #[error(ignore)]
    #[display("{}: expected expression", DisplayAddress(_0))]
    ExpectedExpression(P),
    #[error(ignore)]
    #[display("{}: unclosed tuple", DisplayAddress(_0))]
    UnclosedTuple(P),
    #[error(ignore)]
    #[display("{}: expected identifier", DisplayAddress(_0))]
    ExpectedIdentifier(P),
    #[error(ignore)]
    #[display("{}: expected colon", DisplayAddress(_0))]
    ExpectedColon(P),
    #[error(ignore)]
    #[display("{}: unclosed struct", DisplayAddress(_0))]
    UnclosedStruct(P),
    #[error(ignore)]
    #[display("{}: unclosed list", DisplayAddress(_0))]
    UnclosedList(P),
    #[error(ignore)]
    #[display("{}: expected {keyword} keyword", DisplayAddress(at))]
    ExpectedKeyword { at: P, keyword: &'static str },
    #[error(ignore)]
    #[display("{}: expected path", DisplayAddress(_0))]
    ExpectedPath(P),
    #[error(ignore)]
    #[display("{}: expected semi-colon", DisplayAddress(_0))]
    ExpectedSemiColon(P),
    #[error(ignore)]
    #[display("{}: expected open brace", DisplayAddress(_0))]
    ExpectedOpenBrace(P),
    #[error(ignore)]
    #[display("{}: expected import path", DisplayAddress(_0))]
    ExpectedImportPath(P),
    #[error(ignore)]
    #[display("{}: expected integer (with radix {})", DisplayAddress(_0), _1)]
    ExpectedIntRadix(P, u32),
}

impl<P: SourcePosition> ParseError<P> {
    pub fn map_position<P2: SourcePosition>(self, mut f: impl FnMut(P) -> P2) -> ParseError<P2> {
        match self {
            ParseError::StringError(e) =>
                ParseError::StringError(e.map_position(&mut f)),
            ParseError::ExpectedExpression(p) => ParseError::ExpectedExpression(f(p)),
            ParseError::UnclosedTuple(p) => ParseError::UnclosedTuple(f(p)),
            ParseError::ExpectedIdentifier(p) => ParseError::ExpectedIdentifier(f(p)),
            ParseError::ExpectedColon(p) => ParseError::ExpectedColon(f(p)),
            ParseError::UnclosedStruct(p) => ParseError::UnclosedStruct(f(p)),
            ParseError::UnclosedList(p) => ParseError::UnclosedList(f(p)),
            ParseError::ExpectedKeyword { at, keyword } =>
                ParseError::ExpectedKeyword { at: f(at), keyword },
            ParseError::ExpectedPath(p) => ParseError::ExpectedPath(f(p)),
            ParseError::ExpectedSemiColon(p) => ParseError::ExpectedSemiColon(f(p)),
            ParseError::ExpectedOpenBrace(p) => ParseError::ExpectedOpenBrace(f(p)),
            ParseError::ExpectedImportPath(p) => ParseError::ExpectedImportPath(f(p)),
            ParseError::ExpectedIntRadix(p, r) => ParseError::ExpectedIntRadix(f(p), r),
        }
    }
}

#[derive(Clone)]
pub struct Path<'a>(pub SmallVec<[&'a str; 8]>);

impl<'a> Path<'a> {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn empty() -> Path<'a> {
        Path(SmallVec::new())
    }

    pub fn single(segment: &'a str) -> Path<'a> {
        let mut segments = SmallVec::new();
        segments.push(segment);
        Path(segments)
    }

    pub fn join(&self, other: &Path<'a>) -> Path<'a> {
        let mut segments = SmallVec::with_capacity(self.len() + other.len());
        segments.extend_from_slice(&self.0);
        segments.extend_from_slice(&other.0);
        Path(segments)
    }

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

impl<'a> FromIterator<&'a str> for Path<'a> {
    fn from_iter<T: IntoIterator<Item=&'a str>>(iter: T) -> Self {
        Path(SmallVec::from_iter(iter))
    }
}

impl Display for Path<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.fmt(f)
    }
}

impl Debug for Path<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.fmt(f)
    }
}

#[derive(Clone)]
pub enum Import<'a> {
    Path(Path<'a>),
    File(&'a str),
}

impl Debug for Import<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Import::Path(path) => path.fmt(f),
            Import::File(path) => write!(f, "import {path}"),
        }
    }
}

#[derive(Clone)]
pub enum Number {
    I64(i64),
    U64(u64),
    F64(f64),
}

impl Debug for Number {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Number::I64(v) => write!(f, "{}", v),
            Number::U64(v) => write!(f, "{}", v),
            Number::F64(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Clone)]
pub enum Expression<'a> {
    Number(Number),
    String(&'a str),
    Tuple(Option<Path<'a>>, SmallVec<[usize; 8]>),
    Struct(Option<Path<'a>>, SmallVec<[(&'a str, usize); 8]>),
    List(Option<Path<'a>>, SmallVec<[usize; 8]>),
    Path(Path<'a>),
    Import(Import<'a>),
}

impl<'a> Expression<'a> {
    fn fmt_dot(&self, index: usize, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = dot_register_name(index);
        match self {
            Expression::Tuple(_, body) => {
                for other in body.iter().copied() {
                    writeln!(f, "  {} -> {name};", dot_register_name(other))?;
                }
            }
            Expression::Struct(_, body) => {
                for (_, other) in body.iter().copied() {
                    writeln!(f, "  {} -> {name};", dot_register_name(other))?;
                }
            }
            _ => {},
        }

        Ok(())
    }

    pub fn type_path(&self) -> Option<Path<'a>> {
        match self {
            Expression::Tuple(ty, _) => ty.clone(),
            Expression::Struct(ty, _) => ty.clone(),
            _ => None,
        }
    }
}

impl Debug for Expression<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Expression::Number(s) => write!(f, "{s:?}"),
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
            Expression::List(name, parts) => {
                if let Some(name) = name {
                    write!(f, "{name:?}[")?;
                } else {
                    f.write_char('[')?;
                }

                let mut parts = parts.iter();
                if let Some(part) = parts.next() {
                    write!(f, "%{part}")?;
                    for part in parts {
                        write!(f, ", %{part}")?;
                    }
                }

                f.write_char(']')
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
    pub expression: Option<Expression<'a>>,
}

impl Register<'_> {
    pub(crate) fn fmt_with_index(&self, index: usize, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{index}")?;

        if let Some(name) = &self.name {
            write!(f, " {:?} {}", self.visibility, name)?;
        }

        if let Some(ty) = &self.variable_type {
            let opt = if self.optional { "?" } else { "" };
            write!(f, ": {ty:?}{opt}")?;
        }

        if let Some(expr) = &self.expression {
            write!(f, " = {expr:?}")?;
        }

        Ok(())
    }

    fn fmt_dot(&self, index: usize, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = dot_register_name(index);
        let self_debug = FormatterFn(|f| self.fmt_with_index(index, f)).to_string();
        let label = escape_string(&self_debug);
        writeln!(f, "  {name} [shape=box,label={label}];")?;

        if let Some(expr) = &self.expression {
            expr.fmt_dot(index, f)?;
        }

        Ok(())
    }
}

impl Debug for Register<'_> {
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
            expression: Some(value),
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

    pub fn parse(input: &'a str) -> Result<Document<'a>, ParseError<&'a str>> {
        parse_document(input)
    }

    fn fmt_dot(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "digraph Document {{")?;

        for (index, register) in self.registers.iter().enumerate() {
            register.fmt_dot(index, f)?;
        }

        for (index, a) in self.applications.iter().enumerate() {
            writeln!(f, "  a{index} [label=apply]")?;
            writeln!(f, "  a{index} -> {}", dot_register_name(a.entity))?;
            writeln!(f, "  {} -> a{index}", dot_register_name(a.expression))?;
        }

        writeln!(f, "}}")
    }

    pub fn to_dot(&self) -> String {
        FormatterFn(|f| self.fmt_dot(f)).to_string()
    }

    pub fn dependencies(&self) -> Vec<String> {
        let mut deps = Vec::new();
        for expr in self.registers.iter().filter_map(|r| r.expression.as_ref()) {
            if let Expression::Import(import) = expr {
                match import {
                    Import::Path(_) => {}
                    Import::File(file_path) => {
                        let (_, file_path) = parse_string(file_path).unwrap().unwrap();
                        deps.push(file_path);
                    }
                }
            }
        }

        deps
    }
}

impl Debug for Document<'_> {
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

fn take_while(src: &str, mut f: impl FnMut(char) -> bool) -> (&str, &str) {
    let mut chars = src.chars();
    let mut rest = chars.as_str();
    while let Some(c) = chars.next() {
        if f(c) {
            rest = chars.as_str();
        } else {
            let used = &src[..(src.len() - rest.len())];
            return (rest, used);
        }
    }

    ("", src)
}

fn take_if(src: &str, mut f: impl FnMut(char) -> bool) -> (&str, &str) {
    let mut chars = src.chars();

    if let Some(c) = chars.next() {
        if f(c) {
            let rest = chars.as_str();
            let used = &src[..(src.len() - rest.len())];
            return (rest, used);
        }
    }

    (src, "")
}

fn take_next_if(src: &str, f: impl FnOnce(char) -> bool) -> (&str, bool) {
    let mut chars = src.chars();
    if let Some(c) = chars.next() {
        if f(c) {
            return (chars.as_str(), true);
        }
    }
    (src, false)
}

fn skip_whitespace(mut src: &str) -> &str {
    while !src.is_empty() {
        let mut chars = src.chars();
        match chars.next().unwrap() {
            ' ' | '\t' | '\r' | '\n' => {
                src = chars.as_str();
                continue;
            }
            '/' if src.len() >= 2 => {
                match chars.next().unwrap() {
                    '/' => {
                        src = chars.as_str();
                        match src.split_once('\n') {
                            Some((_, right)) => {
                                src = right;
                                continue;
                            }
                            None => return "",
                        }
                    }
                    '*' => {
                        src = chars.as_str();
                        match src.split_once("*/") {
                            Some((_, right)) => {
                                src = right;
                                continue;
                            }
                            None => return "",
                        }
                    }
                    _ => return src,
                }
            }
            _ => return src,
        }
    }
    src
}

fn int_char_value(c: char) -> Option<u32> {
    let v = match c {
        '0' => 0,
        '1' => 1,
        '2' => 2,
        '3' => 3,
        '4' => 4,
        '5' => 5,
        '6' => 6,
        '7' => 7,
        '8' => 8,
        '9' => 9,
        'A' => 0xa,
        'a' => 0xa,
        'B' => 0xb,
        'b' => 0xb,
        'C' => 0xc,
        'c' => 0xc,
        'D' => 0xd,
        'd' => 0xd,
        'E' => 0xe,
        'e' => 0xe,
        'F' => 0xf,
        'f' => 0xf,
        _ => return None,
    };
    Some(v)
}

fn parse_int_radix(
    src: &str,
    radix: u32,
    negative: bool
) -> Result<(&str, Number), ParseError<&str>> {
    let mut chars = src.chars();
    let mut rest = chars.as_str();
    let mut value: i128 = 0;

    while let Some(next) = chars.next() {
        if next == '_' {
            continue;
        }

        let Some(char_value) = int_char_value(next) else { break };
        if char_value >= radix { break };
        rest = chars.as_str();
        value *= radix as i128;
        value += char_value as i128;
    }

    if rest.len() == src.len() {
        return Err(ParseError::ExpectedIntRadix(src, radix));
    }

    if negative || value == (value as i64 as i128) {
        let mul = if negative { -1 } else { 1 };
        Ok((rest, Number::I64((value as i64) * mul)))
    } else {
        Ok((rest, Number::U64(value as u64)))
    }
}

fn parse_float(src: &str) -> Option<(&str, Number)> {
    fn parse_decimal(src: &str) -> (&str, &str) {
        take_while(src, |c| c.is_ascii_digit() || c == '_')
    }

    let (rest, integer_digits) = parse_decimal(src);
    let needs_digits = !integer_digits.chars().any(|c| c.is_ascii_digit());
    let (rest, has_dot) = take_next_if(rest, |c| c == '.');
    if !has_dot && needs_digits {
        return None;
    }

    let mut rest = rest;
    if has_dot {
        let (next, digits) = parse_decimal(rest);
        rest = next;
        if needs_digits && digits.is_empty() {
            return None;
        }
    }

    let (rest, has_exp) = take_next_if(rest, |c| c == 'e' || c == 'E');
    let mut rest = rest;
    if has_exp {
        let (next, _) = take_if(rest, |c| c == '+' || c == '-');
        let (next, exp_digits) = parse_decimal(next);
        if exp_digits.is_empty() {
            return None;
        }
        rest = next;
    }

    let used = &src[..(src.len() - rest.len())];
    let value = f64::from_str(used).unwrap();
    Some((rest, Number::F64(value)))
}

fn parse_number(src: &str) -> Result<Option<(&str, Number)>, ParseError<&str>> {
    let mut negative = false;
    let mut chars = src.chars();

    let mut peek = chars.clone();
    match peek.next() {
        Some('+') => {
            chars = peek;
        },
        Some('-') => {
            negative = true;
            chars = peek;
        }
        _ => {}
    }

    if chars.next() == Some('0') {
        match chars.next() {
            Some('x') => return Ok(Some(parse_int_radix(chars.as_str(), 16, negative)?)),
            Some('o') => return Ok(Some(parse_int_radix(chars.as_str(), 8, negative)?)),
            Some('b') => return Ok(Some(parse_int_radix(chars.as_str(), 2, negative)?)),
            _ => {}
        }
    }

    if let Some(result) = parse_float(src) {
        return Ok(Some(result));
    }

    Ok(None)
}

fn parse_identifier(src: &str) -> Option<(&str, &str)> {
    let mut chars = src.chars();
    let first = chars.next()?;
    if !first.is_alphabetic() && first != '$' && first != '_' {
        return None;
    }

    let mut rest = chars.as_str();
    while let Some(c) = chars.next() {
        if !c.is_alphanumeric() && c != '$' && c != '_' {
            break;
        }

        rest = chars.as_str();
    }

    let used = &src[..(src.len() - rest.len())];
    Some((rest, used))
}

fn expect_identifier(src: &str) -> Result<(&str, &str), ParseError<&str>> {
    parse_identifier(src).ok_or(ParseError::ExpectedIdentifier(src))
}

fn parse_keyword<'a>(src: &'a str, keyword: &str) -> Option<&'a str> {
    let (rest, kw) = parse_identifier(src)?;
    if kw != keyword {
        None
    } else {
        Some(rest)
    }
}

fn expect_keyword<'a>(src: &'a str, keyword: &'static str) -> Result<&'a str, ParseError<&'a str>> {
    parse_keyword(src, keyword)
        .ok_or(ParseError::ExpectedKeyword { at: src, keyword })
}

fn parse_tuple_body<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<Option<(&'a str, SmallVec<[usize; 8]>)>, ParseError<&'a str>> {
    if !input.starts_with('(') {
        return Ok(None);
    }

    let mut body = SmallVec::new();
    let mut rest = &input[1..];
    while !rest.is_empty() {
        rest = skip_whitespace(rest);
        if rest.starts_with(')') {
            break;
        }

        let (next, expr) = expect_expression_index(document, rest)?;
        body.push(expr);
        rest = skip_whitespace(next);

        if !rest.starts_with(',') {
            break;
        }

        rest = skip_whitespace(&rest[1..]);
    }

    if !rest.starts_with(')') {
        return Err(ParseError::UnclosedTuple(rest));
    }

    Ok(Some((&rest[1..], body)))
}

fn parse_list_body<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<Option<(&'a str, SmallVec<[usize; 8]>)>, ParseError<&'a str>> {
    if !input.starts_with('[') {
        return Ok(None);
    }

    let mut body = SmallVec::new();
    let mut rest = &input[1..];
    while !rest.is_empty() {
        rest = skip_whitespace(rest);
        if rest.starts_with(']') {
            break;
        }

        let (next, expr) = expect_expression_index(document, rest)?;
        body.push(expr);
        rest = skip_whitespace(next);

        if !rest.starts_with(',') {
            break;
        }

        rest = skip_whitespace(&rest[1..]);
    }

    if !rest.starts_with(']') {
        return Err(ParseError::UnclosedList(rest));
    }

    Ok(Some((&rest[1..], body)))
}

fn parse_struct_body<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<Option<(&'a str, SmallVec<[(&'a str, usize); 8]>)>, ParseError<&'a str>> {
    if !input.starts_with('{') {
        return Ok(None);
    }

    let mut body = SmallVec::new();
    let mut rest = &input[1..];
    while !rest.is_empty() {
        rest = skip_whitespace(rest);
        if rest.starts_with('}') {
            break;
        }

        let (next, key) = expect_identifier(rest)?;
        rest = skip_whitespace(next);

        if rest.starts_with([',', '}']) {
            let expr = Expression::Path(Path::single(key));
            let index = document.push_register(expr);
            body.push((key, index));

            if !rest.starts_with(',') {
                break;
            }

            rest = skip_whitespace(&rest[1..]);
            continue;
        } else if !rest.starts_with(':') {
            return Err(ParseError::ExpectedColon(rest));
        }

        rest = skip_whitespace(&rest[1..]);

        let (next, expr) = expect_expression_index(document, rest)?;
        body.push((key, expr));
        rest = skip_whitespace(next);

        if !rest.starts_with(',') {
            break;
        }

        rest = skip_whitespace(&rest[1..]);
    }

    if !rest.starts_with('}') {
        return Err(ParseError::UnclosedStruct(rest));
    }

    Ok(Some((&rest[1..], body)))
}

fn parse_path(src: &str) -> Option<(&str, Path)> {
    let (mut rest, start) = parse_identifier(src)?;
    let mut result = SmallVec::new();
    result.push(start);

    loop {
        rest = skip_whitespace(rest);
        if !rest.starts_with("::") {
            return Some((rest, Path(result)));
        }

        let next = skip_whitespace(&rest[2..]);
        match parse_identifier(next) {
            Some((next, ident)) => {
                result.push(ident);
                rest = next;
            }
            None => {
                return Some((rest, Path(result)));
            }
        }
    }
}

fn expect_path(src: &str) -> Result<(&str, Path), ParseError<&str>> {
    parse_path(src).ok_or(ParseError::ExpectedPath(src))
}

fn parse_path_import<'a>(
    document: &mut Document<'a>,
    input: &'a str,
    root: &Path<'a>,
) -> Result<Option<&'a str>, ParseError<&'a str>> {
    let Some((rest, path)) = parse_path(input) else { return Ok(None) };
    let rest = skip_whitespace(rest);
    let path = root.join(&path);

    if let Some(next) = rest.strip_prefix("::") {
        let rest = skip_whitespace(next);
        if !rest.starts_with('{') {
            return Err(ParseError::ExpectedOpenBrace(rest));
        }
        let mut rest = skip_whitespace(&rest[1..]);

        loop {
            if rest.starts_with('}') {
                rest = skip_whitespace(&rest[1..]);
                return Ok(Some(rest));
            }

            rest = parse_path_import(document, rest, &path)?
                .ok_or(ParseError::ExpectedImportPath(rest))?;
            rest = skip_whitespace(rest);

            if !rest.starts_with(['}', ',']) {
                return Err(ParseError::ExpectedImportPath(rest));
            }

            if rest.starts_with(',') {
                rest = skip_whitespace(&rest[1..]);
            }
        }
    } else {
        let (rest, name) = if let Some(next) = parse_keyword(rest, "as") {
            let rest = skip_whitespace(next);
            expect_identifier(rest)?
        } else {
            (rest, *path.0.last().unwrap())
        };

        document.push_register(Register {
            name: Some(name),
            expression: Some(Expression::Import(Import::Path(path))),
            ..default()
        });
        Ok(Some(rest))
    }
}

fn parse_file_import(input: &str) -> Result<Option<(&str, Register)>, ParseError<&str>> {
    let Some((rest, file_path)) = recognize_string(input)? else { return Ok(None) };
    let rest = skip_whitespace(rest);
    let rest = expect_keyword(rest, "as")?;
    let rest = skip_whitespace(rest);
    let (rest, ident) = expect_identifier(rest)?;

    let expr = Expression::Import(Import::File(file_path));
    let register = Register {
        name: Some(ident),
        expression: Some(expr),
        ..default()
    };

    Ok(Some((rest, register)))
}

fn parse_import<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<Option<&'a str>, ParseError<&'a str>> {
    let Some(rest) = parse_keyword(input, "import") else { return Ok(None) };
    let rest = skip_whitespace(rest);
    match rest.chars().next() {
        Some('"') => {
            let (rest, register) = parse_file_import(rest)?.unwrap();
            document.push_register(register);
            Ok(Some(rest))
        }
        _ => parse_path_import(document, rest, &Path::empty()),
    }
}

fn parse_visibility(input: &str) -> Option<(&str, Visibility)> {
    let (rest, ident) = parse_identifier(input)?;
    let visibility = match ident {
        "in" => Visibility::In,
        "out" => Visibility::Out,
        "local" => Visibility::Local,
        _ => return None,
    };
    Some((rest, visibility))
}

fn parse_variable<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<Option<(&'a str, Register<'a>)>, ParseError<&'a str>> {
    let Some((rest, visibility)) = parse_visibility(input) else { return Ok(None) };
    let rest = skip_whitespace(rest);
    let (rest, name) = expect_identifier(rest)?;
    let rest = skip_whitespace(rest);
    let (rest, has_type) = take_next_if(rest, |c| c == ':');
    let mut rest = skip_whitespace(rest);
    let mut variable_type = None;
    let mut optional = false;
    if has_type {
        let (next, path) = expect_path(rest)?;
        variable_type = Some(path);
        rest = skip_whitespace(next);
        if rest.starts_with('?') {
            optional = true;
            rest = skip_whitespace(&rest[1..]);
        }
    }

    let (rest, has_value) = take_next_if(rest, |c| c == '=');
    let mut rest = skip_whitespace(rest);
    let mut value = None;
    if has_value {
        let (next, expr) = expect_expression(document, rest)?;
        value = Some(expr);
        rest = skip_whitespace(next);
    }

    let register = Register {
        name: Some(name),
        visibility,
        variable_type,
        optional,
        expression: value,
    };
    Ok(Some((rest, register)))
}

fn parse_expression<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<Option<(&'a str, Expression<'a>)>, ParseError<&'a str>> {
    if let Some((rest, v)) = parse_number(input)? {
        let expr = Expression::Number(v);
        return Ok(Some((rest, expr)));
    }

    if let Some((rest, v)) = recognize_string(input)? {
        let expr = Expression::String(v);
        return Ok(Some((rest, expr)));
    }

    match input.chars().next() {
        Some('(') => {
            let (rest, expr) = parse_tuple_body(document, input)?.unwrap();
            let expr = Expression::Tuple(None, expr);
            return Ok(Some((rest, expr)));
        }
        Some('{') => {
            let (rest, expr) = parse_struct_body(document, input)?.unwrap();
            let expr = Expression::Struct(None, expr);
            return Ok(Some((rest, expr)));
        }
        Some('[') => {
            let (rest, expr) = parse_list_body(document, input)?.unwrap();
            let expr = Expression::List(None, expr);
            return Ok(Some((rest, expr)));
        }
        _ => {}
    }

    if let Some((rest, path)) = parse_path(input) {
        let rest = skip_whitespace(rest);
        match rest.chars().next() {
            Some('(') => {
                let (rest, expr) = parse_tuple_body(document, rest)?.unwrap();
                let expr = Expression::Tuple(Some(path), expr);
                return Ok(Some((rest, expr)));
            }
            Some('{') => {
                let (rest, expr) = parse_struct_body(document, rest)?.unwrap();
                let expr = Expression::Struct(Some(path), expr);
                return Ok(Some((rest, expr)));
            }
            Some('[') => {
                let (rest, expr) = parse_list_body(document, rest)?.unwrap();
                let expr = Expression::List(Some(path), expr);
                return Ok(Some((rest, expr)));
            }
            _ => {}
        }

        let expr = Expression::Path(path);
        return Ok(Some((rest, expr)));
    }

    Ok(None)
}

fn expect_expression<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<(&'a str, Expression<'a>), ParseError<&'a str>> {
    parse_expression(document, input)?.ok_or(ParseError::ExpectedExpression(input))
}

fn expect_expression_index<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<(&'a str, usize), ParseError<&'a str>> {
    let (rest, expr) = expect_expression(document, input)?;
    let index = document.push_register(expr);
    Ok((rest, index))
}

fn parse_statement<'a>(
    document: &mut Document<'a>,
    input: &'a str,
) -> Result<Option<&'a str>, ParseError<&'a str>> {
    if let Some(rest) = parse_import(document, input)? {
        return Ok(Some(rest));
    }

    if let Some((rest, register)) = parse_variable(document, input)? {
        document.push_register(register);
        return Ok(Some(rest));
    }

    if let Some((rest, expr)) = parse_expression(document, input)? {
        let rest = skip_whitespace(rest);
        if let Some(next) = rest.strip_prefix("<-") {
            let rest = skip_whitespace(next);
            let expr = document.push_register(expr);
            let (rest, source_expr) = expect_expression_index(document, rest)?;
            document.applications.push(Application {
                entity: expr,
                expression: source_expr,
            });
            return Ok(Some(rest));
        }

        document.push_register(expr);
        return Ok(Some(rest));
    }

    Ok(None)
}


fn parse_document(input: &str) -> Result<Document, ParseError<&str>> {
    let mut document = Document::default();

    let mut rest = input;
    while !rest.is_empty() {
        rest = skip_whitespace(rest);
        if rest.is_empty() {
            break;
        }

        if rest.starts_with(';') {
            rest = skip_whitespace(&rest[1..]);
            continue;
        }

        let Some(next) = parse_statement(&mut document, rest)? else {
            return Err(ParseError::ExpectedExpression(rest));
        };

        rest = skip_whitespace(next);
        if rest.is_empty() {
            break;
        }

        if !rest.starts_with(';') {
            return Err(ParseError::ExpectedSemiColon(rest));
        }
        rest = skip_whitespace(&rest[1..]);
    }

    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build() {
        let doc = Document {
            registers: vec![
                Register {
                    name: Some("var"),
                    visibility: Visibility::In,
                    variable_type: Some(Path::from_iter(["bevy", "Transform"])),
                    optional: true,
                    expression: Some(Expression::Import(Import::Path(Path::single("test2")))),
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
            import foo::{baz::bar, boo::{ham, green as white}};
            import \"jeff.fab\" as mellow;
            in param1: f32 = 5.0;
            in param2: f32? = 0.4;
            out result0: f32;
            local myLocal_: Entity?;
            local test = MyTuple(1, 2, 3.4);
            ;
            $ <- MyComponent {
                field1: 1.0,
                field2: myLocal_,
            };
        ").unwrap();
        let formatted = format!("{doc:?}");
        assert_eq!(formatted,concat!(
        "Document {\n",
        "  %0 local bar = foo::baz::bar;\n",
        "  %1 local ham = foo::boo::ham;\n",
        "  %2 local white = foo::boo::green;\n",
        "  %3 local mellow = import \"jeff.fab\";\n",
        "  %4 in param1: f32 = 5.0;\n",
        "  %5 in param2: f32? = 0.4;\n",
        "  %6 out result0: f32;\n",
        "  %7 local myLocal_: Entity?;\n",
        "  %8 = 1;\n",
        "  %9 = 2;\n",
        "  %10 = 3.4;\n",
        "  %11 local test: MyTuple = MyTuple(%8, %9, %10);\n",
        "  %12 = $;\n",
        "  %13 = 1.0;\n",
        "  %14 = myLocal_;\n",
        "  %15: MyComponent = MyComponent{field1: %13, field2: %14};\n",
        "  %12 <- %15;\n",
        "}",
        ));
    }
}
