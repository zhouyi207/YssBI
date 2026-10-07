//! English words and overlapping Han bigrams with original UTF-8 offsets.

use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

pub(super) const NAME: &str = "yss-mixed-text";

#[derive(Clone)]
pub(super) struct MixedTextTokenizer;

pub(super) struct MixedTextStream {
    tokens: std::vec::IntoIter<Token>,
    current: Token,
}

impl Tokenizer for MixedTextTokenizer {
    type TokenStream<'a> = MixedTextStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> MixedTextStream {
        let mut chars = text.char_indices().peekable();
        let mut tokens = Vec::new();
        while let Some((start, character)) = chars.next() {
            if is_han(character) {
                let mut run = vec![(start, character)];
                while chars
                    .peek()
                    .is_some_and(|(_, character)| is_han(*character))
                {
                    run.push(chars.next().expect("peeked character"));
                }
                if run.len() == 1 {
                    push(&mut tokens, text, start, start + character.len_utf8());
                } else {
                    for pair in run.windows(2) {
                        push(
                            &mut tokens,
                            text,
                            pair[0].0,
                            pair[1].0 + pair[1].1.len_utf8(),
                        );
                    }
                }
            } else if is_word(character) {
                let mut end = start + character.len_utf8();
                while chars
                    .peek()
                    .is_some_and(|(_, character)| is_word(*character))
                {
                    let (offset, character) = chars.next().expect("peeked character");
                    end = offset + character.len_utf8();
                }
                push(&mut tokens, text, start, end);
            }
        }
        MixedTextStream {
            tokens: tokens.into_iter(),
            current: Token::default(),
        }
    }
}

fn push(tokens: &mut Vec<Token>, text: &str, start: usize, end: usize) {
    tokens.push(Token {
        offset_from: start,
        offset_to: end,
        position: tokens.len(),
        text: text[start..end].to_lowercase(),
        position_length: 1,
    });
}

fn is_han(character: char) -> bool {
    matches!(character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' |
        '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
}

fn is_word(character: char) -> bool {
    !is_han(character) && (character.is_alphanumeric() || character == '_')
}

impl TokenStream for MixedTextStream {
    fn advance(&mut self) -> bool {
        if let Some(token) = self.tokens.next() {
            self.current = token;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.current
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.current
    }
}
