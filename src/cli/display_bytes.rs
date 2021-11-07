use std::fmt;

const BYTE_UNITS: &[&str] = &["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

pub struct DisplayBytes {
    unit: Option<&'static str>,
    value: f64,
    padded: bool,
}

impl fmt::Display for DisplayBytes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(unit) = self.unit {
            write!(f, "{:.2} {}", self.value, unit)
        } else if self.padded {
            write!(f, "{:.0} B ", self.value)
        } else {
            write!(f, "{:.0} B", self.value)
        }
    }
}

impl DisplayBytes {
    pub fn new(value: u64) -> Self {
        let value = value as f64;

        if let Some((value, unit)) = BYTE_UNITS
            .iter()
            .enumerate()
            .map(|(i, u)| (value / 1000_f64.powf(i as f64 + 1.0), u))
            .take_while(|(i, _)| *i > 1.0)
            .last()
        {
            Self {
                unit: Some(unit),
                value,
                padded: false,
            }
        } else {
            Self {
                unit: None,
                value,
                padded: false,
            }
        }
    }

    pub fn new_padded(value: u64) -> Self {
        let value = value as f64;

        if let Some((value, unit)) = BYTE_UNITS
            .iter()
            .enumerate()
            .map(|(i, u)| (value / 1000_f64.powf(i as f64 + 1.0), u))
            .take_while(|(i, _)| *i > 1.0)
            .last()
        {
            Self {
                unit: Some(unit),
                value,
                padded: true,
            }
        } else {
            Self {
                unit: None,
                value,
                padded: true,
            }
        }
    }
}
