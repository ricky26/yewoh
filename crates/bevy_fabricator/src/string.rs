use nom::branch::alt;
use nom::character::complete::{anychar, char, one_of};
use nom::combinator::{map, not, recognize};
use nom::error::ParseError;
use nom::multi::{fold_many1, many1};
use nom::sequence::{delimited, pair};
use nom::IResult;

enum StringFragment<'a> {
    Literal(&'a str),
    EscapedChar(char),
}

fn parse_fragment<'a, E>(src: &'a str) -> IResult<&'a str, StringFragment<'a>, E>
where
    E: ParseError<&'a str>,
{
    alt((
        map(recognize(many1(not(one_of("\\\"")))), StringFragment::Literal),
        map(pair(char('\\'), anychar), |(_, c)| StringFragment::EscapedChar(c)),
    ))(src)
}

pub fn recognize_string<'a, E>(src: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    recognize(delimited(
        char('"'),
        many1(parse_fragment),
        char('"'),
    ))(src)
}

pub fn parse_string<'a, E>(src: &'a str) -> IResult<&'a str, String, E>
where
    E: ParseError<&'a str>,
{
    delimited(
        char('"'),
        fold_many1(parse_fragment, || String::new(), |mut acc, v| {
            match v {
                StringFragment::Literal(s) => {
                    acc.push_str(s);
                }
                StringFragment::EscapedChar(c) => {
                    let c = match c {
                        'r' => '\r',
                        'n' => '\n',
                        c => c,
                    };
                    acc.push(c);
                }
            }
            acc
        }),
        char('"'),
    )(src)
}
