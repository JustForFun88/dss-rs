use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

mod rpn;

pub use rpn::RPNCalculator;

// Custom error type for parser problems
#[derive(Debug)]
pub struct ParserError {
    message: String,
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParserError {}

impl ParserError {
    pub fn new(message: &str) -> Self {
        ParserError {
            message: message.to_string(),
        }
    }
}

// Parser Variables
#[derive(Debug)]
pub struct ParserVar {
    variables: HashMap<String, String>,
    active_variable: Option<String>,
}

impl ParserVar {
    pub fn new() -> Self {
        let mut variables = HashMap::new();

        // Add intrinsic variables
        variables.insert("@lastfile".to_string(), "null".to_string());
        variables.insert("@lastexportfile".to_string(), "null".to_string());
        variables.insert("@lastshowfile".to_string(), "null".to_string());
        variables.insert("@lastplotfile".to_string(), "null".to_string());
        variables.insert("@lastredirectfile".to_string(), "null".to_string());
        variables.insert("@lastcompilefile".to_string(), "null".to_string());
        variables.insert("@result".to_string(), "null".to_string());

        ParserVar {
            variables,
            active_variable: None,
        }
    }

    pub fn add(&mut self, var_name: &str, var_value: &str) -> bool {
        let var_definition = if var_value.contains('@') {
            format!("{{{}}}", var_value)
        } else {
            var_value.to_string()
        };

        self.variables.insert(var_name.to_string(), var_definition);
        true
    }

    pub fn lookup(&mut self, var_name: &str) -> bool {
        if self.variables.contains_key(var_name) {
            self.active_variable = Some(var_name.to_string());
            true
        } else {
            self.active_variable = None;
            false
        }
    }

    pub fn get_value(&self) -> String {
        if let Some(ref var_name) = self.active_variable {
            self.variables.get(var_name).cloned().unwrap_or_default()
        } else {
            String::new()
        }
    }

    pub fn set_value(&mut self, value: &str) {
        if let Some(ref var_name) = self.active_variable {
            self.variables.insert(var_name.clone(), value.to_string());
        }
    }

    pub fn get_var_string(&self, var_name: &str) -> String {
        if let Some(value) = self.variables.get(var_name) {
            let display_value = if value.is_empty() { "null" } else { value };
            format!("{}. {}", var_name, display_value)
        } else {
            "Variable not found".to_string()
        }
    }

    pub fn num_variables(&self) -> usize {
        self.variables.len()
    }
}

// Main DSS Parser
#[derive(Debug)]
pub struct DSSParser {
    parser_vars: Option<ParserVar>,
    cmd_buffer: String,
    position: usize,
    parameter_buffer: String,
    token_buffer: String,
    delim_chars: String,
    whitespace_chars: String,
    begin_quote_chars: String,
    end_quote_chars: String,
    last_delimiter: char,
    matrix_row_terminator: char,
    auto_increment: bool,
    convert_error: bool,
    is_quoted_string: bool,
    rpn_calculator: RPNCalculator,
}

impl DSSParser {
    pub const COMMENT_CHAR: char = '!';
    pub const VARIABLE_DELIMITER: char = '@'; // first character of a variable

    pub fn new() -> Self {
        DSSParser {
            parser_vars: None,
            cmd_buffer: String::new(),
            position: 0,
            parameter_buffer: String::new(),
            token_buffer: String::new(),
            delim_chars: ",=".to_string(),
            whitespace_chars: " \t".to_string(),
            begin_quote_chars: "(\"'[{".to_string(),
            end_quote_chars: ")}']".to_string(),
            last_delimiter: ' ',
            matrix_row_terminator: '|',
            auto_increment: false,
            convert_error: false,
            is_quoted_string: false,
            rpn_calculator: RPNCalculator::new(),
        }
    }

    // pub fn set_vars(&mut self, vars: ParserVar) {
    //     self.parser_vars = Some(vars);
    // }

    // pub fn set_cmd_string(&mut self, value: &str) {
    //     self.cmd_buffer = format!("{} ", value); // add whitespace at end
    //     self.position = 0;
    //     self.skip_whitespace();
    // }

    // pub fn reset_delims(&mut self) {
    //     self.delim_chars = ",=".to_string();
    //     self.whitespace_chars = " \t".to_string();
    //     self.matrix_row_terminator = '|';
    //     self.begin_quote_chars = "(\"'[{".to_string();
    //     self.end_quote_chars = ")}']".to_string();
    // }

    fn is_whitespace(&self, ch: char) -> bool {
        self.whitespace_chars.contains(ch)
    }

    fn is_delim_char(&self, ch: char) -> bool {
        self.delim_chars.contains(ch)
    }

    fn is_comment_char(&self, ch: char, next_ch: Option<char>) -> bool {
        ch == '!' || (ch == '/' && next_ch == Some('/'))
    }

    fn skip_whitespace(&mut self) {
        let chars: Vec<char> = self.cmd_buffer.chars().collect();
        while self.position < chars.len() && self.is_whitespace(chars[self.position]) {
            self.position += 1;
        }
    }

    fn is_delimiter(&self, ch: char, next_ch: Option<char>) -> bool {
        self.is_comment_char(ch, next_ch) || self.is_delim_char(ch) || self.is_whitespace(ch)
    }

    fn get_token(&mut self) -> String {
        let chars: Vec<char> = self.cmd_buffer.chars().collect();

        if self.position >= chars.len() {
            return String::new();
        }

        self.is_quoted_string = false;
        let ch = chars[self.position];

        // Check for quotes
        if let Some(quote_pos) = self.begin_quote_chars.find(ch) {
            let end_quote = self.end_quote_chars.chars().nth(quote_pos).unwrap();
            self.position += 1;
            let start = self.position;

            while self.position < chars.len() && chars[self.position] != end_quote {
                self.position += 1;
            }

            let token = chars[start..self.position].iter().collect();
            if self.position < chars.len() {
                self.position += 1; // skip end quote
            }
            self.is_quoted_string = true;
            return token;
        }

        // Parse regular token
        let start = self.position;
        let next_ch = if self.position + 1 < chars.len() {
            Some(chars[self.position + 1])
        } else {
            None
        };

        while self.position < chars.len() && !self.is_delimiter(chars[self.position], next_ch) {
            self.position += 1;
        }

        let token: String = chars[start..self.position].iter().collect();

        // Handle delimiter
        if self.position < chars.len() {
            self.last_delimiter = chars[self.position];

            if self.is_comment_char(chars[self.position], next_ch) {
                self.position = chars.len(); // Skip to end on comment
            } else {
                if self.is_delim_char(chars[self.position]) {
                    self.position += 1;
                }
                self.skip_whitespace();
            }
        }

        token
    }

    // fn check_for_var(&mut self, token: &mut String) -> bool {
    //     if token.len() <= 1 || !token.starts_with('@') {
    //         return false;
    //     }

    //     let original_token = token.clone();

    //     // Find dot or caret position
    //     let dot_pos = token.find('.').or_else(|| token.find('^'));

    //     let var_name = if let Some(pos) = dot_pos {
    //         &token[..pos]
    //     } else {
    //         token
    //     };

    //     if let Some(ref mut vars) = self.parser_vars {
    //         if vars.lookup(var_name) {
    //             let var_value = vars.get_value();

    //             if var_value.starts_with('{') && var_value.ends_with('}') {
    //                 let inner_value = &var_value[1..var_value.len() - 1];
    //                 if let Some(pos) = dot_pos {
    //                     *token = format!("{}{}", inner_value, &token[pos..]);
    //                 } else {
    //                     *token = inner_value.to_string();
    //                 }
    //                 self.is_quoted_string = true;
    //             } else {
    //                 if let Some(pos) = dot_pos {
    //                     *token = format!("{}{}", var_value, &token[pos..]);
    //                 } else {
    //                     *token = var_value;
    //                 }
    //             }
    //             return true;
    //         }
    //     }

    //     *token = original_token;
    //     false
    // }

    fn check_for_var(&mut self) -> bool {
        let original_token = self.token_buffer.clone();

        if self.token_buffer.len() > 1 && self.token_buffer.starts_with(Self::VARIABLE_DELIMITER) {
            let delimiter_pos = self
                .token_buffer
                .find('^')
                .or_else(|| self.token_buffer.find('.'));

            let variable_name = if let Some(pos) = delimiter_pos {
                &self.token_buffer[..pos]
            } else {
                &self.token_buffer
            };

            if let Some(ref mut vars) = self.parser_vars {
                if vars.lookup(variable_name) {
                    let var_value = vars.get_value();

                    if var_value.starts_with('{') && var_value.ends_with('}') {
                        let inner_value = &var_value[1..var_value.len() - 1];
                        self.token_buffer = if let Some(pos) = delimiter_pos {
                            format!("{}{}", inner_value, &self.token_buffer[pos..])
                        } else {
                            inner_value.to_string()
                        };
                        self.is_quoted_string = true;
                    } else {
                        self.token_buffer = if let Some(pos) = delimiter_pos {
                            format!("{}{}", var_value, &self.token_buffer[pos..])
                        } else {
                            var_value
                        };
                    }
                }
            }
        }

        self.token_buffer == original_token
    }

    pub fn next_param(&mut self) -> String {
        if self.position < self.cmd_buffer.len() {
            self.last_delimiter = ' ';
            self.token_buffer = self.get_token();

            if self.last_delimiter == '=' {
                self.parameter_buffer = self.token_buffer.clone();
                self.token_buffer = self.get_token();
            } else {
                self.parameter_buffer.clear();
            }
        } else {
            self.parameter_buffer.clear();
            self.token_buffer.clear();
        }

        self.check_for_var();
        self.parameter_buffer.clone()
    }

    // pub fn parse_as_bus_name(&mut self, param: &str) -> (String, Vec<i32>) {
    //     self.token_buffer = param.to_string();

    //     if self.auto_increment {
    //         self.next_param();
    //     }

    //     let mut nodes = Vec::new();

    //     if let Some(dot_pos) = self.token_buffer.find('.') {
    //         let bus_name = self.token_buffer[..dot_pos].trim().to_string();
    //         let node_part = &self.token_buffer[dot_pos + 1..];

    //         for node_str in node_part.split('.') {
    //             if let Ok(node) = node_str.parse::<i32>() {
    //                 nodes.push(node);
    //             } else {
    //                 nodes.push(-1); // Error indicator
    //             }
    //         }

    //         (bus_name, nodes)
    //     } else {
    //         (self.token_buffer.clone(), nodes)
    //     }
    // }

    // pub fn parse_as_vector(&mut self, expected_size: usize) -> Vec<f64> {
    //     if self.auto_increment {
    //         self.next_param();
    //     }

    //     let mut vector = vec![0.0; expected_size];
    //     let mut elements_found = 0;

    //     let parse_buffer = format!("{} ", self.token_buffer);
    //     let mut parse_pos = 0;
    //     let chars: Vec<char> = parse_buffer.chars().collect();

    //     while parse_pos < chars.len() {
    //         // Skip whitespace
    //         while parse_pos < chars.len() && chars[parse_pos].is_whitespace() {
    //             parse_pos += 1;
    //         }

    //         if parse_pos >= chars.len() {
    //             break;
    //         }

    //         // Get token
    //         let start = parse_pos;
    //         while parse_pos < chars.len()
    //             && !chars[parse_pos].is_whitespace()
    //             && chars[parse_pos] != self.matrix_row_terminator
    //         {
    //             parse_pos += 1;
    //         }

    //         if start == parse_pos {
    //             break;
    //         }

    //         let token: String = chars[start..parse_pos].iter().collect();

    //         if elements_found < expected_size {
    //             if let Ok(value) = token.parse::<f64>() {
    //                 vector[elements_found] = value;
    //             }
    //         }

    //         elements_found += 1;

    //         if parse_pos < chars.len() && chars[parse_pos] == self.matrix_row_terminator {
    //             break;
    //         }
    //     }

    //     vector
    // }

    // pub fn make_string(&mut self) -> String {
    //     if self.auto_increment {
    //         self.next_param();
    //     }
    //     self.token_buffer.clone()
    // }

    // pub fn make_integer(&mut self) -> Result<i32, ParserError> {
    //     self.convert_error = false;

    //     if self.auto_increment {
    //         self.next_param();
    //     }

    //     if self.token_buffer.is_empty() {
    //         return Ok(0);
    //     }

    //     if self.is_quoted_string {
    //         let value = self.interpret_rpn_string()?;
    //         return Ok(value.round() as i32);
    //     }

    //     // Try direct conversion
    //     if let Ok(value) = self.token_buffer.parse::<i32>() {
    //         return Ok(value);
    //     }

    //     // Try as float then round
    //     if let Ok(value) = self.token_buffer.parse::<f64>() {
    //         return Ok(value.round() as i32);
    //     }

    //     self.convert_error = true;
    //     Err(ParserError::new(&format!(
    //         "Integer number conversion error for string: \"{}\"",
    //         self.token_buffer
    //     )))
    // }

    // pub fn make_double(&mut self) -> Result<f64, ParserError> {
    //     self.convert_error = false;

    //     if self.auto_increment {
    //         self.next_param();
    //     }

    //     if self.token_buffer.is_empty() {
    //         return Ok(0.0);
    //     }

    //     if self.is_quoted_string {
    //         return self.interpret_rpn_string();
    //     }

    //     match self.token_buffer.parse::<f64>() {
    //         Ok(value) => Ok(value),
    //         Err(_) => {
    //             self.convert_error = true;
    //             Err(ParserError::new(&format!(
    //                 "Floating point number conversion error for string: \"{}\"",
    //                 self.token_buffer
    //             )))
    //         }
    //     }
    // }

    // fn interpret_rpn_string(&mut self) -> Result<f64, ParserError> {
    //     let parse_buffer = format!("{} ", self.token_buffer);
    //     let mut parse_pos = 0;
    //     let chars: Vec<char> = parse_buffer.chars().collect();

    //     while parse_pos < chars.len() {
    //         // Skip whitespace
    //         while parse_pos < chars.len() && chars[parse_pos].is_whitespace() {
    //             parse_pos += 1;
    //         }

    //         if parse_pos >= chars.len() {
    //             break;
    //         }

    //         // Get token
    //         let start = parse_pos;
    //         while parse_pos < chars.len() && !chars[parse_pos].is_whitespace() {
    //             parse_pos += 1;
    //         }

    //         let token: String = chars[start..parse_pos].iter().collect();
    //         self.process_rpn_command(&token)?;
    //     }

    //     Ok(self.rpn_calculator.get_x())
    // }

    // fn process_rpn_command(&mut self, token: &str) -> Result<(), ParserError> {
    //     // Try to parse as number first
    //     if let Ok(number) = token.parse::<f64>() {
    //         self.rpn_calculator.set_x(number);
    //         return Ok(());
    //     }

    //     // Process RPN commands
    //     match token.to_lowercase().as_str() {
    //         "+" => self.rpn_calculator.add(),
    //         "-" => self.rpn_calculator.subtract(),
    //         "*" => self.rpn_calculator.multiply(),
    //         "/" => self.rpn_calculator.divide(),
    //         "sqrt" => self.rpn_calculator.sqrt(),
    //         "sqr" => self.rpn_calculator.square(),
    //         "^" => self.rpn_calculator.y_to_the_x_power(),
    //         "sin" => self.rpn_calculator.sin_deg(),
    //         "cos" => self.rpn_calculator.cos_deg(),
    //         "tan" => self.rpn_calculator.tan_deg(),
    //         "asin" => self.rpn_calculator.asin_deg(),
    //         "acos" => self.rpn_calculator.acos_deg(),
    //         "atan" => self.rpn_calculator.atan_deg(),
    //         "atan2" => self.rpn_calculator.atan2_deg(),
    //         "swap" => self.rpn_calculator.swap_xy(),
    //         "rollup" => self.rpn_calculator.roll_up(),
    //         "rolldn" => self.rpn_calculator.roll_down(),
    //         "ln" => self.rpn_calculator.natlog(),
    //         "pi" => self.rpn_calculator.enter_pi(),
    //         "log10" => self.rpn_calculator.ten_log(),
    //         "exp" => self.rpn_calculator.etothex(),
    //         "inv" => self.rpn_calculator.inv(),
    //         _ => {
    //             return Err(ParserError::new(&format!(
    //                 "Invalid inline math entry: \"{}\"",
    //                 token
    //             )));
    //         }
    //     }

    //     Ok(())
    // }

    // pub fn get_remainder(&self) -> String {
    //     if self.position < self.cmd_buffer.len() {
    //         self.cmd_buffer[self.position..].to_string()
    //     } else {
    //         String::new()
    //     }
    // }

    // Getters and setters
    // pub fn get_token(&self) -> &str {
    //     &self.token_buffer
    // }

    // pub fn set_token(&mut self, token: &str) {
    //     self.token_buffer = token.to_string();
    // }

    // pub fn get_position(&self) -> usize {
    //     self.position
    // }

    // pub fn set_position(&mut self, pos: usize) {
    //     self.position = pos;
    // }

    // pub fn get_delimiters(&self) -> &str {
    //     &self.delim_chars
    // }

    // pub fn set_delimiters(&mut self, delims: &str) {
    //     self.delim_chars = delims.to_string();
    // }

    // pub fn get_auto_increment(&self) -> bool {
    //     self.auto_increment
    // }

    // pub fn set_auto_increment(&mut self, auto_inc: bool) {
    //     self.auto_increment = auto_inc;
    // }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_basic_parsing() {
//         let mut parser = DSSParser::new();
//         parser.set_cmd_string("param1=value1 param2=value2");

//         let param1 = parser.next_param();
//         assert_eq!(param1, "param1");
//         assert_eq!(parser.get_token(), "value1");

//         let param2 = parser.next_param();
//         assert_eq!(param2, "param2");
//         assert_eq!(parser.get_token(), "value2");
//     }

//     #[test]
//     fn test_rpn_calculator() {
//         let mut calc = RPNCalculator::new();
//         calc.set_x(5.0);
//         calc.set_x(3.0);
//         calc.add();
//         assert_eq!(calc.get_x(), 8.0);
//     }

//     #[test]
//     fn test_variable_parsing() {
//         let mut vars = ParserVar::new();
//         vars.add("@myvar", "42");

//         let mut parser = DSSParser::new();
//         parser.set_vars(vars);
//         parser.set_cmd_string("@myvar");

//         parser.next_param();
//         let result = parser.make_integer().unwrap();
//         assert_eq!(result, 42);
//     }

//     #[test]
//     fn test_bus_name_parsing() {
//         let mut parser = DSSParser::new();
//         let (bus_name, nodes) = parser.parse_as_bus_name("Bus1.1.2.3");
//         assert_eq!(bus_name, "Bus1");
//         assert_eq!(nodes, vec![1, 2, 3]);
//     }
// }
