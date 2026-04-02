use std::time::Instant;

use gleanmark_core::embedding::EmbeddingService;

#[tokio::main]
async fn main() {
    let text_short = "Rust is a systems programming language focused on safety and performance";
    let text_long = "Rust is a systems programming language focused on safety and performance. ".repeat(50); // ~3500 chars
    let text_zh_long = "機器學習是人工智慧的一個分支，透過演算法讓電腦從資料中學習。深度學習利用多層神經網路來處理複雜的模式識別任務。".repeat(20); // ~2400 chars

    println!("Initializing models...");
    let t = Instant::now();
    let svc = EmbeddingService::new().unwrap();
    println!("Init: {:.1}s\n", t.elapsed().as_secs_f64());

    for (label, text) in [
        ("Short EN (70 chars)", text_short.to_string()),
        ("Long EN (~3500 chars)", text_long),
        ("Long ZH (~2400 chars)", text_zh_long),
    ] {
        println!("--- {label} ---");

        let t = Instant::now();
        let _ = svc.embed_passage(&text).await.unwrap();
        println!("embed_passage: {}ms", t.elapsed().as_millis());

        let t = Instant::now();
        let _ = svc.embed_query(&text).await.unwrap();
        println!("embed_query:   {}ms", t.elapsed().as_millis());

        println!();
    }
}
