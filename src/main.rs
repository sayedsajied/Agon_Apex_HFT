use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use serde_json::{json, Value};

#[tokio::main]
async fn main() {
    println!("🚀 AGON APEX HFT CORE - STARTING...");
    println!("⚙️ ENGINE 1: Delta Exchange WebSocket Connecting...");

    let url = Url::parse("wss://socket.delta.exchange").unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("❌ Connection Failed!");
    println!("✅ Connected! Sending payload...");

    let (mut write, mut read) = ws_stream.split();

    // BTC/USDT का पेलोड
    let subscribe_msg = json!({
        "type": "subscribe",
        "payload": {
            "channels": [
                {
                    "name": "v2/ticker",
                    "symbols": ["BTCUSDT"]
                }
            ]
        }
    });

    use futures_util::SinkExt;
    write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
    println!("📡 Subscribed! Filtering raw data for Lightning Fast Execution...\n");
    println!("--------------------------------------------------");

    // यह लूप कचरा हटाकर सिर्फ काम के 3 नंबर निकालेगा
    while let Some(message) = read.next().await {
        match message {
            Ok(msg) => {
                if msg.is_text() {
                    let text = msg.into_text().unwrap();
                    
                    // JSON डेटा को डिकोड करना
                    if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                        // सिर्फ वो डेटा निकालो जिसमें प्राइस है
                        if let (Some(symbol), Some(price), Some(bid), Some(ask)) = (
                            parsed.get("symbol").and_then(|s| s.as_str()),
                            parsed.get("mark_price").and_then(|p| p.as_str()),
                            parsed.get("best_bid").and_then(|b| b.as_str()),
                            parsed.get("best_ask").and_then(|a| a.as_str())
                        ) {
                            println!("🎯 [{}]: PRICE: {} | BID: {} | ASK: {}", symbol, price, bid, ask);
                        }
                    }
                }
            }
            Err(e) => {
                println!("⚠️ Error: {:?}", e);
                break;
            }
        }
    }
}
