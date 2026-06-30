//! Bus-name and vector/matrix parsing (Pascal `ParseAsBusName`/`ParseAsVector`/
//! `ParseAsMatrix`/`ParseAsSymMatrix`).

use crate::vars::ParserVars;

use super::Parser;
use super::error::ParserError;

impl Parser {
    /// Split `"busname.1.2.3"` into the bus name and its node numbers
    /// (Pascal `ParseAsBusName`). Without a dot the whole token is the name
    /// (untrimmed, like the original); with nodes the name is trimmed.
    pub fn parse_as_bus_name(
        &mut self,
        param: &str,
        vars: &ParserVars,
    ) -> Result<(String, Vec<i32>), ParserError> {
        self.token_buffer = param.to_string();
        if self.auto_increment {
            self.next_param(vars);
        }
        let Some(dot_pos) = self.token_buffer.find('.') else {
            return Ok((self.token_buffer.clone(), Vec::new()));
        };
        let name = self.token_buffer[..dot_pos].trim().to_string();
        let token_save = std::mem::take(&mut self.token_buffer);
        let node_buffer = format!("{} ", &token_save[dot_pos + 1..]);

        let delim_save = std::mem::replace(&mut self.delim_chars, ".".to_string());
        let mut nodes = Vec::new();
        let mut pos = 0usize;
        let mut error = None;

        self.token_buffer = self.get_token_at(&node_buffer, &mut pos);
        while !self.token_buffer.is_empty() {
            match self.make_integer(vars) {
                Ok(v) => nodes.push(if self.convert_error { -1 } else { v }),
                Err(e) => {
                    error = Some(e);
                    break;
                }
            }
            self.token_buffer = self.get_token_at(&node_buffer, &mut pos);
        }

        self.delim_chars = delim_save; // restore original delimiters
        self.token_buffer = token_save;
        match error {
            Some(e) => Err(e),
            None => Ok((name, nodes)),
        }
    }

    /// Parse the current token as a vector of doubles into `out`
    /// (Pascal `ParseAsVector`). Returns the number of elements *found* —
    /// which may exceed `out.len()`; the extras are consumed but dropped.
    /// Scanning stops at the matrix row terminator `|`, leaving the rest of
    /// the token for the next row. With `do_round` each stored element is
    /// rounded ties-to-even (Pascal `DoRound`).
    pub fn parse_as_vector(
        &mut self,
        vars: &ParserVars,
        out: &mut [f64],
        do_round: bool,
    ) -> Result<usize, ParserError> {
        if self.auto_increment {
            self.next_param(vars);
        }
        let mut num_elements = 0usize;
        out.fill(0.0);

        let parse_buffer = format!("{} ", self.token_buffer);
        let mut pos = 0usize;
        let delim_save = self.delim_chars.clone();
        self.delim_chars.push(self.matrix_row_terminator as char);
        let mut error = None;

        self.skip_white_space(&parse_buffer, &mut pos);
        self.token_buffer = self.get_token_at(&parse_buffer, &mut pos);
        self.check_for_var(vars);
        while !self.token_buffer.is_empty() {
            num_elements += 1;
            if num_elements <= out.len() {
                match self.make_double(vars) {
                    Ok(v) => out[num_elements - 1] = v,
                    Err(e) => {
                        error = Some(e);
                        break;
                    }
                }
            }
            if self.last_delimiter == self.matrix_row_terminator {
                break;
            }
            self.token_buffer = self.get_token_at(&parse_buffer, &mut pos);
            self.check_for_var(vars);
        }

        self.delim_chars = delim_save; // restore original delimiters
        // prepare for the next trip (the following matrix row)
        self.token_buffer = parse_buffer.get(pos..).unwrap_or("").to_string();
        if do_round {
            let stored = num_elements.min(out.len());
            for v in out[..stored].iter_mut() {
                *v = v.round_ties_even();
            }
        }
        match error {
            Some(e) => Err(e),
            None => Ok(num_elements),
        }
    }

    /// Parse `order` rows separated by `|` into a full matrix in
    /// column-major (Fortran) order (Pascal `ParseAsMatrix`).
    /// Returns `order` on success.
    pub fn parse_as_matrix(
        &mut self,
        vars: &ParserVars,
        out: &mut [f64],
        order: usize,
    ) -> Result<usize, ParserError> {
        if self.auto_increment {
            self.next_param(vars);
        }
        let mut row_buf = vec![0.0; order];
        out[..order * order].fill(0.0);

        for i in 0..order {
            let elements_found = self.parse_as_vector(vars, &mut row_buf, false)?;
            if elements_found > order * order {
                return Err(ParserError::new(
                    "Matrix Buffer in ParseAsMatrix too small. Check your input data, \
                     especially dimensions and number of phases."
                        .to_string(),
                ));
            }
            // Pascal read past RowBuf for elements beyond the order
            // (undefined behavior there); the extras are ignored here.
            for j in 0..elements_found.min(order) {
                out[j * order + i] = row_buf[j];
            }
        }
        Ok(order)
    }

    /// Parse a lower-triangle-by-rows symmetric matrix into a full
    /// column-major matrix with optional element `stride` and `scale`
    /// (Pascal `ParseAsSymMatrix`). Returns `order` on success.
    pub fn parse_as_sym_matrix(
        &mut self,
        vars: &ParserVars,
        out: &mut [f64],
        order: usize,
        stride: usize,
        scale: f64,
    ) -> Result<usize, ParserError> {
        if self.auto_increment {
            self.next_param(vars);
        }
        let mut row_buf = vec![0.0; order];
        let maxpos = order * order - 1;
        for i in 0..order * order {
            out[i * stride] = 0.0;
        }

        for i in 0..order {
            let elements_found = self.parse_as_vector(vars, &mut row_buf, false)?;
            // A range loop on purpose: when a row has more elements than the
            // order, the subpos check below must error out BEFORE row_buf[j]
            // is read (the Pascal code's exact behavior); an iterator would
            // silently stop at the buffer end instead.
            #[allow(clippy::needless_range_loop)]
            for j in 0..elements_found {
                let subpos = j * order + i;
                if subpos > maxpos {
                    return Err(ParserError::new(
                        "Matrix Buffer in ParseAsSymMatrix too small. Check your input \
                         data, especially dimensions and number of phases."
                            .to_string(),
                    ));
                }
                out[subpos * stride] = row_buf[j] * scale;

                if i == j {
                    continue;
                }
                let subpos = i * order + j;
                if subpos > maxpos {
                    return Err(ParserError::new(
                        "Matrix Buffer in ParseAsSymMatrix too small. Check your input \
                         data, especially dimensions and number of phases."
                            .to_string(),
                    ));
                }
                out[subpos * stride] = row_buf[j] * scale;
            }
        }
        Ok(order)
    }
}
