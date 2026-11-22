use std::collections::HashMap;
use crate::FormatError;

pub const DICT_PHRASE_LEN: usize = 32;
pub const MAX_DICT_PHRASES: usize = 254;
pub const FILTER_SHARED_DICT: u8 = 0x20;

pub fn train_seed_dictionary(samples: &[Vec<u8>]) -> Vec<u8> {
    let mut freq: HashMap<[u8; DICT_PHRASE_LEN], usize> = HashMap::new();

    for sample in samples {
        let limit = sample.len().min(64 * 1024);
        let mut offset = 0usize;
        while offset + DICT_PHRASE_LEN <= limit {
            let mut phrase = [0u8; DICT_PHRASE_LEN];
            phrase.copy_from_slice(&sample[offset..offset + DICT_PHRASE_LEN]);
            if !phrase.iter().all(|&b| b == phrase[0]) {
                *freq.entry(phrase).or_insert(0) += 1;
            }
            offset += DICT_PHRASE_LEN;
        }
    }

    let mut candidates: Vec<([u8; DICT_PHRASE_LEN], usize)> =
        freq.into_iter().filter(|(_, count)| *count >= 2).collect();
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.truncate(MAX_DICT_PHRASES);

    let mut dict = Vec::with_capacity(candidates.len() * DICT_PHRASE_LEN);
    for (phrase, _) in candidates {
        dict.extend_from_slice(&phrase);
    }
    dict
}

pub fn apply_dict_transform(data: &[u8], dict: &[u8]) -> Vec<u8> {
    let num_phrases = dict.len() / DICT_PHRASE_LEN;
    if num_phrases == 0 || data.len() < DICT_PHRASE_LEN {
        return data.to_vec();
    }

    let mut lookup: HashMap<&[u8], u8> = HashMap::with_capacity(num_phrases);
    for idx in 0..num_phrases.min(MAX_DICT_PHRASES) {
        let start = idx * DICT_PHRASE_LEN;
        lookup.insert(&dict[start..start + DICT_PHRASE_LEN], idx as u8);
    }

    let mut out = Vec::with_capacity(data.len());
    let mut i = 0usize;
    let n = data.len();

    while i < n {
        if i + DICT_PHRASE_LEN <= n {
            let window = &data[i..i + DICT_PHRASE_LEN];
            if let Some(&idx) = lookup.get(window) {
                out.push(0xFF);
                out.push(idx);
                i += DICT_PHRASE_LEN;
                continue;
            }
        }
        let b = data[i];
        if b == 0xFF {
            out.push(0xFF);
            out.push(0xFF);
        } else {
            out.push(b);
        }
        i += 1;
    }

    out
}

pub fn reverse_dict_transform(transformed: &[u8], dict: &[u8]) -> Result<Vec<u8>, FormatError> {
    let num_phrases = dict.len() / DICT_PHRASE_LEN;
    if num_phrases == 0 {
        return Ok(transformed.to_vec());
    }

    let mut out = Vec::with_capacity(transformed.len() * 2);
    let mut i = 0usize;
    let n = transformed.len();

    while i < n {
        let b = transformed[i];
        if b == 0xFF {
            if i + 1 >= n {
                return Err(FormatError::CorruptedBlock(
                    "Truncated dictionary escape sequence".to_string(),
                ));
            }
            let code = transformed[i + 1];
            if code == 0xFF {
                out.push(0xFF);
            } else {
                let idx = code as usize;
                if idx >= num_phrases {
                    return Err(FormatError::CorruptedBlock(format!(
                        "Invalid dictionary phrase index {}",
                        idx
                    )));
                }
                let start = idx * DICT_PHRASE_LEN;
                out.extend_from_slice(&dict[start..start + DICT_PHRASE_LEN]);
            }
            i += 2;
        } else {
            out.push(b);
            i += 1;
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seed_dictionary_training_and_lossless_roundtrip() {
        let header_phrase = b"import React, { useState, useEffect } from 'react';\n";
        let mut f1 = header_phrase.repeat(10);
        f1.extend_from_slice(&[0xFF, 0x00, 0xFE, 0xFF, 0x42]);
        let f2 = header_phrase.repeat(12);

        let dict = train_seed_dictionary(&[f1.clone(), f2]);
        assert!(dict.len() >= DICT_PHRASE_LEN);

        let transformed = apply_dict_transform(&f1, &dict);
        assert!(transformed.len() < f1.len());

        let restored = reverse_dict_transform(&transformed, &dict).unwrap();
        assert_eq!(restored, f1);
    }
}