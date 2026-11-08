//! Generator plików referencyjnych (ręcznie): zapisuje zabawkowy model (`testkit`) do katalogu
//! `ALFA_EMBED_DUMP_DIR` — wejście dla `tests/fixtures/gen_references.py` (onnxruntime + HF `tokenizers`).
//!
//! `ALFA_EMBED_DUMP_DIR=/tmp/toy cargo test -p lib-embed --test it dump -- --ignored`

#[test]
#[ignore = "generator referencji — uruchamiany ręcznie"]
fn dump_toy_models() {
    let Ok(dir) = std::env::var("ALFA_EMBED_DUMP_DIR") else {
        return;
    };
    use lib_embed::testkit::{
        TOY_DIMS, TOY_VOCAB, toy_model_bytes, write_toy_model, xlmr_like_model_bytes,
    };
    let dir = std::path::PathBuf::from(dir);
    write_toy_model(&dir).unwrap();
    let types = toy_model_bytes(TOY_VOCAB, TOY_DIMS, true);
    std::fs::write(dir.join("model-types.onnx"), types).unwrap();
    std::fs::write(dir.join("xlmr-like.onnx"), xlmr_like_model_bytes(1502)).unwrap();
    eprintln!("zapisano w {}", dir.display());
}
