use ansi_term::Style;
use pad::{Alignment, PadStr};
use std::fmt::{Display, Error, Formatter};
use unicode_width::UnicodeWidthStr;

pub struct Table {
    columns: usize,
    cells: Vec<Cell>,
}

impl Table {
    pub fn new(columns: usize) -> Table {
        Table {
            columns,
            cells: Vec::new(),
        }
    }

    pub fn add_cell(&mut self, text: &str) {
        self.cells.push(Cell {
            text: text.to_string(),
            align: Alignment::Left,
        });
    }

    pub fn add_cell_rtl(&mut self, text: &str) {
        self.cells.push(Cell {
            text: text.to_string(),
            align: Alignment::Right,
        });
    }
}

impl Display for Table {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        if self.cells.len() == 0 {
            return Ok(());
        }

        let column_sizes: Vec<_> = (0..self.columns)
            .map(|i| {
                self.cells
                    .iter()
                    .skip(i)
                    .step_by(self.columns)
                    .map(|x| UnicodeWidthStr::width(x.text.as_str()))
                    .max()
                    .unwrap()
            })
            .collect();

        for (i, cell) in self.cells.iter().enumerate() {
            let c = i % self.columns;
            let size = column_sizes[c];

            if i < self.columns {
                write!(
                    f,
                    "{}",
                    Style::new()
                        .bold()
                        .paint(&cell.text.pad_to_width_with_alignment(size, cell.align))
                )?;
            } else {
                write!(
                    f,
                    "{}",
                    cell.text.pad_to_width_with_alignment(size, cell.align)
                )?;
            };

            if c == self.columns - 1 {
                writeln!(f)?;
            } else {
                write!(f, " ")?;
            }
        }

        Ok(())
    }
}

struct Cell {
    text: String,
    align: Alignment,
}
