use flate2::read::GzDecoder;
use indexmap::{IndexMap, IndexSet};
use log::warn;
use once_cell::sync::Lazy;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::Mutex;
use tract_itertools::Itertools;
use tract_onnx::prelude::*;


// adapted from https://github.com/openai/CLIP/blob/main/clip/simple_tokenizer.py

const CONTEXT_LENGTH: usize = 77;

static TOKENIZER: Lazy<Mutex<SimpleTokenizer>> = Lazy::new(|| {
    let bpe_gzip_vocab = include_bytes!("clip/vocab.txt.gz");
    let mut gz = GzDecoder::new(&bpe_gzip_vocab[..]);
    let mut bpe_vocab = String::new();
    gz.read_to_string(&mut bpe_vocab).unwrap();
    Mutex::new(SimpleTokenizer::new(&bpe_vocab))
});

/// input is a slice of all possible tags (they are all encoded in bulk)
pub fn tokenize(input: &[String]) -> anyhow::Result<Tensor> {
    let mut tokenizer = TOKENIZER.lock().unwrap();
    let sot_token = tokenizer.encoder["<|startoftext|>"];
    let eot_token = tokenizer.encoder["<|endoftext|>"];

    let all_tokens: Result<Vec<Vec<usize>>, anyhow::Error> = input
        .iter()
        .map(|text| {
            Ok(vec![sot_token]
                .into_iter()
                .chain(tokenizer.encode(text)?.into_iter())
                .take(CONTEXT_LENGTH - 1) // always truncate
                .chain(vec![eot_token])
                .collect())
        })
        .collect();
    let all_tokens = all_tokens?;

    let result: Tensor = tract_ndarray::Array2::from_shape_fn(
        (all_tokens.len(), CONTEXT_LENGTH),
        |(input_idx, token_idx)| {
            let tokens = all_tokens.get(input_idx).unwrap(); // must exist
            let token = tokens.get(token_idx);
            if let Some(token) = token {
                *token as i64
            } else {
                0_i64
            }
        },
    )
    .into();

    Ok(result)
}

fn bytes_to_unicode() -> IndexMap<u32, char> {
    let mut bs = Vec::new();
    bs.extend(
        (('!' as u32)..=('~' as u32))
            .chain(('¡' as u32)..=('¬' as u32))
            .chain(('®' as u32)..=('ÿ' as u32)),
    );
    let mut cs = bs.clone();
    let mut n = 0;
    for b in 0..256 {
        if !bs.contains(&b) {
            bs.push(b);
            cs.push(256 + n);
            n += 1;
        }
    }
    let cs = cs.into_iter().map(|c| char::from_u32(c).unwrap());
    bs.into_iter().zip(cs).collect()
}

fn get_pairs(word: &Vec<String>) -> IndexSet<(String, String)> {
    word.into_iter()
        .tuple_windows()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn basic_clean(text: &str) -> String {
    // TODO: idk if that is necessary
    // let fixed_text = ftfy::fix_text(text);
    // html::decode_entities(&html::decode_entities(&fixed_text)).to_string().trim().to_string()
    text.to_string()
}

fn whitespace_clean(text: &str) -> String {
    let cleaned_text = regex::Regex::new(r"\s+")
        .unwrap()
        .replace_all(text, " ")
        .to_string();
    cleaned_text.trim().to_string()
}

struct SimpleTokenizer {
    byte_encoder: IndexMap<u32, char>,
    byte_decoder: IndexMap<char, u32>,
    encoder: IndexMap<String, usize>,
    decoder: IndexMap<usize, String>,
    bpe_ranks: IndexMap<(String, String), usize>,
    cache: IndexMap<String, String>,
    pat: regex::Regex,
    vocab: Vec<String>,
}

impl SimpleTokenizer {
    fn new(bpe_vocab: &str) -> Self {
        let byte_encoder = bytes_to_unicode();
        let byte_decoder: IndexMap<char, u32> =
            byte_encoder.iter().map(|(&b, &c)| (c, b)).collect();

        let merges = bpe_vocab
            .split("\n")
            .skip(1)
            .take(49152 - 256 - 2)
            .collect_vec();
        let merges = merges
            .iter()
            .map(|merge| {
                let merge = merge.split_whitespace().collect_vec();
                (merge[0].to_string(), merge[1].to_string())
            })
            .collect_vec();

        let vocab = bytes_to_unicode();
        let vocab = vocab.values().collect_vec();
        let vocab = vocab
            .iter()
            .map(|v| v.to_string())
            .chain(vocab.iter().map(|v| format!("{v}</w>")))
            .collect_vec();
        let vocab = vocab
            .into_iter()
            .chain(merges.iter().map(|merge| format!("{}{}", merge.0, merge.1)))
            .chain(["<|startoftext|>".to_string(), "<|endoftext|>".to_string()].into_iter())
            .collect_vec();

        let encoder: IndexMap<_, _> = vocab.iter().cloned().zip(0..vocab.len()).collect();
        let decoder: IndexMap<_, _> = encoder.iter().map(|(k, &v)| (v, k.clone())).collect();
        let bpe_ranks: IndexMap<_, _> = merges.iter().cloned().zip(0..merges.len()).collect();

        let mut cache = IndexMap::new();
        cache.insert("<|startoftext|>".to_string(), "<|startoftext|>".to_string());
        cache.insert("<|endoftext|>".to_string(), "<|endoftext|>".to_string());

        let pat = regex::Regex::new(r#"(?i)<\|startoftext\|>|<\|endoftext\|>|'s|'t|'re|'ve|'m|'ll|'d|[\p{L}]+|[\p{N}]|[^\s\p{L}\p{N}]+"#).unwrap();

        SimpleTokenizer {
            byte_encoder,
            byte_decoder,
            encoder,
            decoder,
            bpe_ranks,
            cache,
            pat,
            vocab, // not needed except for tests
        }
    }

    fn bpe(&mut self, token: &str) -> anyhow::Result<String> {
        if let Some(cached) = self.cache.get(token) {
            return Ok(cached.clone());
        }
        let mut word = token.chars().map(|c| c.to_string()).collect_vec();
        let last = word.pop().unwrap();
        word.push(format!("{last}</w>"));

        let mut pairs = get_pairs(&word);

        if pairs.is_empty() {
            return Ok(format!("{token}</w>"));
        }

        loop {
            let bigram = pairs
                .iter()
                .min_by_key(|&pair| self.bpe_ranks.get(pair).unwrap_or(&usize::MAX)) // TODO: float::inf?
                .unwrap();
            if !self.bpe_ranks.contains_key(bigram) {
                break;
            }
            let (first, second) = bigram;
            let mut new_word = Vec::new();
            let mut i = 0;
            while i < word.len() {
                {
                    let j = word.iter().positions(|w| w == first).find(|idx| idx >= &i);
                    if let Some(j) = j {
                        new_word.extend_from_slice(&word[i..j]);
                        i = j;
                    } else {
                        new_word.extend_from_slice(&word[i..]);
                        break;
                    }
                }

                if word[i] == *first && i < word.len() - 1 && word[i + 1] == *second {
                    new_word.push(format!("{first}{second}"));
                    i += 2;
                } else {
                    new_word.push(word[i].clone());
                    i += 1;
                }
            }
            word = new_word;
            if word.len() == 1 {
                break;
            }
            pairs = get_pairs(&word);
        }

        let word = word.iter().join("");
        self.cache.insert(token.to_string(), word.clone());
        Ok(word)
    }

    fn encode(&mut self, text: &str) -> anyhow::Result<Vec<usize>> {
        let mut bpe_tokens = Vec::new();
        let cleaned_text = whitespace_clean(&basic_clean(text)).to_lowercase();

        for token in self.pat.clone().find_iter(&cleaned_text) {
            let token: String = token
                .as_str()
                .chars()
                .map(|c| self.byte_encoder[&(c as u32)])
                .collect();
            let tokens = self.bpe(&token)?;
            let tokens = tokens.split(" ").collect_vec();
            let result: Vec<Result<_, _>> = tokens
                .iter()
                .map(|bpe_token| {
                    self.encoder
                        .get(*bpe_token)
                        .ok_or(anyhow::anyhow!("unknown token {bpe_token}"))
                })
                .collect_vec();
            let result: Result<Vec<_>, _> = result.into_iter().collect();
            bpe_tokens.extend(result?)
        }
        Ok(bpe_tokens)
    }

    // fn decode(&self, tokens: Vec<usize>) -> String {
    //     let text = tokens
    //         .into_iter()
    //         .map(|token| self.decoder[&token].clone())
    //         .join("");
    //     text.chars()
    //         .map(|c| char::from_u32(self.byte_decoder[&c]).unwrap())
    //         .collect()
    //     // TODO: replace "</w>"" with " "
    // }
}

#[cfg(test)]
mod tests {
    use tract_onnx::tract_hir::tract_ndarray::Axis;

    use super::*;

    #[test]
    fn test_internal() {
        let text = "dog.".to_string();
        let mut tokenizer = TOKENIZER.lock().unwrap();
        assert_eq!(tokenizer.bpe("dog").unwrap(), "dog</w>");
        assert_eq!(tokenizer.bpe(".").unwrap(), ".</w>");

        assert_eq!(49408, tokenizer.vocab.len());

        let vocab_start: Vec<String> = serde_json::from_str(r##"["!", "\"", "#", "$", "%", "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", ":", ";", "<", "=", ">", "?", "@", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "[", "\\", "]", "^", "_", "`", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "{", "|", "}", "~", "\u00a1", "\u00a2", "\u00a3", "\u00a4", "\u00a5", "\u00a6", "\u00a7", "\u00a8", "\u00a9", "\u00aa", "\u00ab", "\u00ac", "\u00ae", "\u00af", "\u00b0", "\u00b1", "\u00b2", "\u00b3", "\u00b4", "\u00b5", "\u00b6", "\u00b7", "\u00b8", "\u00b9", "\u00ba", "\u00bb", "\u00bc", "\u00bd", "\u00be", "\u00bf", "\u00c0", "\u00c1", "\u00c2", "\u00c3", "\u00c4", "\u00c5", "\u00c6", "\u00c7", "\u00c8", "\u00c9", "\u00ca", "\u00cb", "\u00cc", "\u00cd", "\u00ce", "\u00cf", "\u00d0", "\u00d1", "\u00d2", "\u00d3", "\u00d4", "\u00d5", "\u00d6", "\u00d7", "\u00d8", "\u00d9", "\u00da", "\u00db", "\u00dc", "\u00dd", "\u00de", "\u00df", "\u00e0", "\u00e1", "\u00e2", "\u00e3", "\u00e4", "\u00e5", "\u00e6", "\u00e7", "\u00e8", "\u00e9", "\u00ea", "\u00eb", "\u00ec", "\u00ed", "\u00ee", "\u00ef", "\u00f0", "\u00f1", "\u00f2", "\u00f3", "\u00f4", "\u00f5", "\u00f6", "\u00f7", "\u00f8", "\u00f9", "\u00fa", "\u00fb", "\u00fc", "\u00fd", "\u00fe", "\u00ff", "\u0100", "\u0101", "\u0102", "\u0103", "\u0104", "\u0105", "\u0106", "\u0107", "\u0108", "\u0109", "\u010a", "\u010b", "\u010c", "\u010d", "\u010e", "\u010f", "\u0110", "\u0111", "\u0112", "\u0113", "\u0114", "\u0115", "\u0116", "\u0117", "\u0118", "\u0119", "\u011a", "\u011b", "\u011c", "\u011d", "\u011e", "\u011f", "\u0120", "\u0121", "\u0122", "\u0123", "\u0124", "\u0125", "\u0126", "\u0127", "\u0128", "\u0129", "\u012a", "\u012b", "\u012c", "\u012d", "\u012e", "\u012f", "\u0130", "\u0131", "\u0132", "\u0133", "\u0134", "\u0135", "\u0136", "\u0137", "\u0138", "\u0139", "\u013a", "\u013b", "\u013c", "\u013d", "\u013e", "\u013f", "\u0140", "\u0141", "\u0142", "\u0143", "!</w>", "\"</w>", "#</w>", "$</w>", "%</w>", "&</w>", "'</w>", "(</w>", ")</w>", "*</w>", "+</w>", ",</w>", "-</w>", ".</w>", "/</w>", "0</w>", "1</w>", "2</w>", "3</w>", "4</w>", "5</w>", "6</w>", "7</w>", "8</w>", "9</w>", ":</w>", ";</w>", "<</w>", "=</w>", "></w>", "?</w>", "@</w>", "A</w>", "B</w>", "C</w>", "D</w>", "E</w>", "F</w>", "G</w>", "H</w>", "I</w>", "J</w>", "K</w>", "L</w>"]"##).unwrap();
        let vocab_end: Vec<String> = serde_json::from_str(r##"["asiangames</w>", "campeon", "appropriation</w>", "thcentury</w>", "ramatta</w>", "draped</w>", "bullion</w>", "muc</w>", "onex</w>", "segreg", "ophelia</w>", "bodily</w>", "\u00e2\u013f\u00a4\u00f0\u0141\u013a\u012f</w>", "wizar", "teased</w>", "ademy</w>", "toid</w>", "sura</w>", "lazarus</w>", "snickers</w>", "mase", "loh", "bowed</w>", "biblio", "xchange</w>", "harlan</w>", "ghoshal</w>", "flavorful</w>", "bhagat</w>", "allez</w>", "whichever</w>", "tenstein</w>", "discer", "organiser</w>", "mtg", "dreamliner</w>", "tse", "hokkaido</w>", "mok", "indulgent</w>", "hickman</w>", "blinded</w>", "alyn", "aaaah</w>", "spool</w>", "loughborough</w>", "interpret", "etv", "aristotle</w>", "optimizing</w>", "avicii</w>", "madurai</w>", "juli</w>", "nawaz", "matchups</w>", "abide</w>", "painting", "welling</w>", "veli</w>", "octagon</w>", "inscribed</w>", "poking</w>", "placer</w>", "lifecycle</w>", "kilig</w>", "gsp</w>", "elives</w>", "clements</w>", "nasheed</w>", "mesut</w>", "incarcerated</w>", "distilled</w>", "walang</w>", "delicacy</w>", "delgado</w>", "chez", "chita</w>", "adero</w>", "tux</w>", "patil</w>", "odo", "abhcosmetics</w>", "tvc</w>", "pbc</w>", "inaccurate</w>", "hardworkpaysoff</w>", "baller", "quotation</w>", "merchandising</w>", "gastri", "defenses</w>", "drogba</w>", "bexhill</w>", "bankno", "winona</w>", "sieg", "pgs</w>", "hahahha</w>", "aguchi</w>", "subram", "miracle", "desch", "libre", "bacher</w>", "entine</w>", "bbcradi", "loudest</w>", "rps</w>", "pierc", "fryer</w>", "stormtrooper</w>", "rafaelnadal</w>", "pasco</w>", "exhaustion</w>", "epiconetsy</w>", "rctid</w>", "kellie</w>", "gaines</w>", "dbz</w>", "smriti", "sbridge</w>", "limited", "claw", "technical", "biographical</w>", "adored</w>", "\u00e0\u00b8\u00b0</w>", "exclude</w>", "acadia</w>", "keyboards</w>", "furman</w>", "soca</w>", "suru</w>", "nips</w>", "swaps</w>", "serverless</w>", "rune</w>", "puffy</w>", "northampton", "nishings</w>", "hender", "cartridges</w>", "gunshot</w>", "\u00f0\u0141\u0135\u00b9</w>", "filament</w>", "respondents</w>", "peyton", "mountaineer</w>", "merging</w>", "lifespan</w>", "intimidation</w>", "pafc</w>", "nlwx</w>", "expansive</w>", "purr", "fck</w>", "cae</w>", "atti", "telethon</w>", "sohn</w>", "mendel", "lopes</w>", "dori</w>", "unbroken</w>", "tered", "tastings</w>", "inactive</w>", "disintegr", "tassel</w>", "sharethe", "piano", "islay</w>", "airspace</w>", "zawa</w>", "ricciardo</w>", "mington", "fresher</w>", "curry", "revs</w>", "pharoah</w>", "hmv</w>", "exhilarating</w>", "whoo</w>", "linkin</w>", "krispy</w>", "competency</w>", "stewards</w>", "nebu", "katsu", "admins</w>", "bazar</w>", "asar</w>", "givingback</w>", "ssummit</w>", "songz</w>", "linus</w>", "rajkumar</w>", "farmington</w>", "fantasia</w>", "\u00f0\u0141\u013a\u00b4\u00f0\u0141\u013a\u00b4</w>", "sobri", "lisse</w>", "barrymore</w>", "prism", "blob</w>", "senew", "monoxide</w>", "expire</w>", "eighteen</w>", "dipper</w>", "xiao</w>", "kilt</w>", "hinch", "bbcsport</w>", "bamboo", "pter", "exal", "\u00f0\u0141\u00a6\u012d", "hamlin</w>", "expeditions</w>", "stargazing</w>", "foodsecurity</w>", "wylie</w>", "ulf</w>", "stingly</w>", "onstorm</w>", "loeb</w>", "broome</w>", "bnha</w>", "pancreatic</w>", "elive", "!!!!!!!!!!!</w>", "therapper</w>", "orthopedic</w>", "avengersendgame</w>", "antitrust</w>", "\u00ec\u013c\u00b0</w>", "gote</w>", "omd</w>", "offside</w>", "gyllen", "wineries</w>", "whitewater</w>", "adl</w>", "lupita</w>", "exceeds</w>", "consisted</w>", "chewbacca</w>", "ashleigh</w>", "nhljets</w>", "issan", "shld</w>", "hayat</w>", "cranberries</w>", "\u00f0\u0141\u00a4\u013a\u00f0\u0141\u0131\u00bd</w>", "rockthe", "springtraining</w>", "fallout", "dairyfree</w>", "waj</w>", "undecided</w>", "sown</w>", "rcn</w>", "northwales</w>", "httr</w>", "fumble</w>", "dits</w>", "compelled</w>", "populist</w>", "minted</w>", "blanchett</w>", ".''</w>", "propulsion</w>", "milla</w>", "auberg", "hertz</w>", "hta</w>", "udaipur</w>", "serendipity</w>", "aztecs</w>", "alsace</w>", "\u00f0\u0141\u0132\u0133</w>", "lun</w>", "shoes", "charli</w>", "garza</w>", "\u00f0\u0141\u0134\u0141", "probiotics</w>", "foxtv</w>", "olis</w>", "miff", "localized</w>", "diffuser</w>", "sigue</w>", "funko", "rendous</w>", "\u00f0\u0141\u0134\u0133</w>", "jekyll</w>", "<|startoftext|>", "<|endoftext|>"]"##).unwrap();

        itertools::assert_equal(vocab_start.iter(), tokenizer.vocab[..300].iter());
        itertools::assert_equal(
            vocab_end.iter(),
            tokenizer.vocab[(tokenizer.vocab.len() - 300)..].iter(),
        );

        assert_eq!(tokenizer.encoder["dog</w>"], 1929);
        assert_eq!(tokenizer.encoder[".</w>"], 269);
    }

    #[test]
    fn test_encode_word() {
        let text = "dog.".to_string();
        let mut tokenizer = TOKENIZER.lock().unwrap();
        let tokens = tokenizer.encode(&text);
        assert_eq!(tokens.unwrap(), vec![1929, 269]);
    }

    #[test]
    fn test_encode_sentence() {
        let text = "The quick brown fox jumps over the lazy dog.".to_string();
        let mut tokenizer = TOKENIZER.lock().unwrap();
        let tokens = tokenizer.encode(&text);

        assert_eq!(
            tokens.unwrap(),
            vec![518, 3712, 2866, 3240, 18911, 962, 518, 10753, 1929, 269]
        );
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize(&["a test picture".to_string(), "tokens".to_string()]).unwrap();
        let arr = tokens.to_array_view::<i64>().unwrap();
        let arrays = arr
            .axis_iter(Axis(0))
            .map(|arr| arr.into_iter().collect_vec())
            .collect_vec();

        assert_eq!(arrays.len(), 2);
        assert_eq!(arrays[0].len(), 77);
        assert_eq!(arrays[1].len(), 77);

        itertools::assert_equal(
            arrays[0][0..6].into_iter().cloned().cloned(),
            vec![49406, 320, 1628, 1674, 49407, 0].into_iter(),
        );
        itertools::assert_equal(
            arrays[1][0..4].into_iter().cloned().cloned(),
            vec![49406, 23562, 49407, 0].into_iter(),
        );
    }
}
