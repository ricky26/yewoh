use std::fmt::{Display, Formatter, Write};

static DIGITS_TO_PLACE_VALUE: [i64; 19] = [
    1,
    10,
    100,
    1000,
    10000,
    100000,
    1000000,
    10000000,
    100000000,
    1000000000,
    10000000000,
    100000000000,
    1000000000000,
    10000000000000,
    100000000000000,
    1000000000000000,
    10000000000000000,
    100000000000000000,
    1000000000000000000,
];

fn place_value(value: i64, place: usize) -> u32 {
    ((value / DIGITS_TO_PLACE_VALUE[place]) % 10) as u32
}

fn place_char(value: i64, place: usize) -> char {
    char::from_digit(place_value(value, place), 10).unwrap()
}

#[derive(Clone, Copy, Debug)]
pub struct FormatInteger(pub i64);

impl From<i64> for FormatInteger {
    fn from(value: i64) -> Self {
        FormatInteger(value)
    }
}

impl From<u64> for FormatInteger {
    fn from(value: u64) -> Self {
        FormatInteger(value as i64)
    }
}

impl From<i32> for FormatInteger {
    fn from(value: i32) -> Self {
        FormatInteger(value as i64)
    }
}

impl From<u32> for FormatInteger {
    fn from(value: u32) -> Self {
        FormatInteger(value as i64)
    }
}

impl From<i16> for FormatInteger {
    fn from(value: i16) -> Self {
        FormatInteger(value as i64)
    }
}

impl From<u16> for FormatInteger {
    fn from(value: u16) -> Self {
        FormatInteger(value as i64)
    }
}

impl From<i8> for FormatInteger {
    fn from(value: i8) -> Self {
        FormatInteger(value as i64)
    }
}

impl From<u8> for FormatInteger {
    fn from(value: u8) -> Self {
        FormatInteger(value as i64)
    }
}

impl Display for FormatInteger {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            return f.write_char('0');
        }

        let mut v= self.0;
        if v < 0 {
            f.write_char('-')?;
            v = -v;
        }

        let num_digits = self.0.ilog10() as usize;
        f.write_char(place_char(v, num_digits))?;

        for digit in (0..num_digits).rev() {
            if (digit % 3) == 2 {
                f.write_char(',')?;
            }

            f.write_char(place_char(v, digit))?;
        }

        Ok(())
    }
}
