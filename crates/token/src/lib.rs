/*!
This crate provides the means to calculate the exact amount of tokens an openAI will calculate
given some text. OpenAI breaks text into pieces called tokens. The cost of request is determined
by he tokens sent + tokens received. Tokens are calculated in the following manner:
1 token ~= 4 chars in English
1 token ~= 0.75 words
 */

mod bpe;

use base64::{engine, Engine as _};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io;
use std::io::Read;

use bpe::BPE;

///
/// This function returns the average token count between words and characters.
pub fn count(mut reader: impl io::BufRead) -> Result<u32, Box<dyn Error>> {
    let encoding = BPE {
        encoder: mergeable_ranks()?,
        special_tokens: HashMap::from([
            ("<|endoftext|>".to_string(), 100257),
            ("<|fim_prefix|>".to_string(), 100257),
            ("<|fim_middle|>".to_string(), 100257),
            ("<|fim_suffix|>".to_string(), 100257),
            ("<|endofprompt|>".to_string(), 100257),
        ]),
        pattern: r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+"
            .to_string(),
    };
    let mut text = String::new();
    let allowed_specials = HashSet::from([("")]);

    reader
        .read_to_string(&mut text)
        .expect("Unable to read file");

    let token_count = encoding.encode(&text, &allowed_specials);

    Ok(token_count.len() as u32)
}

fn mergeable_ranks() -> Result<HashMap<Vec<u8>, usize>, Box<dyn Error>> {
    let mut token_ranks: HashMap<Vec<u8>, usize> = Default::default();
    let mut cl100k_ranks =
        ureq::get("https://openaipublic.blob.core.windows.net/encodings/cl100k_base.tiktoken")
            .call()?
            .into_string()?;
    for line in cl100k_ranks.lines() {
        let line_split: Vec<&str> = line.split(" ").collect();
        let token: Vec<u8> = engine::general_purpose::STANDARD
            .decode(line_split[0])
            .unwrap();
        let rank: usize = line_split[1].parse().unwrap();
        token_ranks.insert(token, rank);
    }
    Ok(token_ranks)
}
