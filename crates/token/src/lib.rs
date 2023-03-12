/*!
This crate provides the means to calculate the exact amount of tokens an openAI will calculate
given some text. OpenAI breaks text into pieces called tokens. The cost of request is determined
by he tokens sent + tokens received. Tokens are calculated in the following manner:
1 token ~= 4 chars in English
1 token ~= 0.75 words
 */

use std::error::Error;
use std::io;

///
/// This function returns the average token count between words and characters.
pub fn count(mut reader: impl io::BufRead) -> Result<u32, Box<dyn Error>> {
    let mut num_chars = 0;
    let mut num_words = 0;
    let mut line = String::new();

    loop {
        let line_bytes = reader.read_line(&mut line)?;
        if line_bytes == 0 {
            break;
        }
        num_words += line.split_whitespace().count();
        num_chars += line.chars().count();
        line.clear()
    }
    let token_chars = num_chars as f32 / 2.0;
    let token_words = num_words as f32 / 0.5;
    let token_average = (token_words + token_chars) as u32 / 2;
    Ok(token_average)
}
