use std::time::Instant;

use gleanmark_core::embedding::EmbeddingService;

#[tokio::main]
async fn main() {
    let text_en = "Rust is a systems programming language focused on safety and performance";
    let text_zh = "機器學習是人工智慧的一個分支，透過演算法讓電腦從資料中學習";

    println!("Initializing models...");
    let t = Instant::now();
    let svc = EmbeddingService::new().unwrap();
    println!("Init: {:.1}s\n", t.elapsed().as_secs_f64());

    for (label, text) in [("English", text_en), ("Chinese", text_zh)] {
        println!("--- {label} ---");

        let t = Instant::now();
        let _ = svc.embed_passage(text).await.unwrap();
        println!("embed_passage: {:.1}ms", t.elapsed().as_millis());

        let t = Instant::now();
        let _ = svc.embed_query(text).await.unwrap();
        println!("embed_query:   {:.1}ms", t.elapsed().as_millis());

        println!();
    }
}
