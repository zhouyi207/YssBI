//! English words and overlapping Han bigrams with original UTF-8 offsets.

use std::iter::Peekable;
use std::str::CharIndices;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

pub(super) const NAME: &str = "yss-mixed-text";

#[derive(Clone)]
pub(super) struct MixedTextTokenizer;

pub(super) struct MixedTextStream<'a> {
    text: &'a str,
    chars: Peekable<CharIndices<'a>>,
    han_tail: Option<(usize, char)>,
    next_position: usize,
    current: Token,
}

impl Tokenizer for MixedTextTokenizer {
    type TokenStream<'a> = MixedTextStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> MixedTextStream<'a> {
        MixedTextStream {
            text,
            chars: text.char_indices().peekable(),
            han_tail: None,
            next_position: 0,
            current: Token::default(),
        }
    }
}

fn is_han(character: char) -> bool {
    matches!(character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' |
        '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
}

fn is_word(character: char) -> bool {
    !is_han(character) && (character.is_alphanumeric() || character == '_')
}

impl TokenStream for MixedTextStream<'_> {
    fn advance(&mut self) -> bool {
        loop {
            let Some((start, character)) = self.han_tail.take().or_else(|| self.chars.next())
            else {
                return false;
            };
            let mut end = start + character.len_utf8();
            if is_han(character) {
                if self.chars.peek().is_some_and(|(_, next)| is_han(*next)) {
                    let (offset, next) = self.chars.next().expect("peeked character");
                    end = offset + next.len_utf8();
                    // Retain only the overlapping character; a completed run
                    // must not emit its last character again as a single token.
                    if self.chars.peek().is_some_and(|(_, next)| is_han(*next)) {
                        self.han_tail = Some((offset, next));
                    }
                }
            } else if is_word(character) {
                while self.chars.peek().is_some_and(|(_, next)| is_word(*next)) {
                    let (offset, next) = self.chars.next().expect("peeked character");
                    end = offset + next.len_utf8();
                }
            } else {
                continue;
            }
            self.current = Token {
                offset_from: start,
                offset_to: end,
                position: self.next_position,
                text: self.text[start..end].to_lowercase(),
                position_length: 1,
            };
            self.next_position += 1;
            return true;
        }
    }

    fn token(&self) -> &Token {
        &self.current
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_text_preserves_overlaps_word_boundaries_and_utf8_offsets() {
        let mut tokenizer = MixedTextTokenizer;
        let mut stream = tokenizer.token_stream("ΣΟΣ🧪中 AB_2汉字 汉字词 文𠀀A");
        let mut actual = Vec::new();
        while stream.advance() {
            let token = stream.token();
            assert_eq!(token.position_length, 1);
            actual.push((
                token.text.clone(),
                token.offset_from,
                token.offset_to,
                token.position,
            ));
        }
        let expected = [
            ("σος", 0, 6, 0),
            ("中", 10, 13, 1),
            ("ab_2", 14, 18, 2),
            ("汉字", 18, 24, 3),
            ("汉字", 25, 31, 4),
            ("字词", 28, 34, 5),
            ("文𠀀", 35, 42, 6),
            ("a", 42, 43, 7),
        ]
        .map(|(text, start, end, position)| (text.to_owned(), start, end, position));
        assert_eq!(actual, expected);
    }
}
