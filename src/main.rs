use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use serde_json::{json, Value};

#[tokio::main]
async fn main() {
    println!("🚀 AGON APEX HFT CORE - THE TRIFECTA ENGINE STARTING...");
    println!("⚙️ WARM-UP: Connecting to Delta Exchange...");

    let url = Url::parse("wss://socket.delta.exchange").unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("❌ Connection Failed!");
    println!("✅ Connected! Firing Final Payload for BTC, ETH & SOL...");

    let (mut write, mut read) = ws_stream.split();

    // तीनों कॉइन्स (BTC, ETH, SOL) का एक साथ रॉ पेलोड
    let subscribe_msg = json!({
        "type": "subscribe",
        "payload": {
            "channels": [
                {
                    "name": "v2/ticker",
                    "symbols": ["BTCUSDT", "ETHUSDT", "SOLUSDT"] 
                }
            ]
        }
    });

    use futures_util::SinkExt;
    write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
    println!("📡 Subscribed to BTC, ETH & SOL! Pulling RAW Data Streams...\n");
    println!("========================================================");

    // तीनों इंजनों का रॉ टिक-डेटा (Raw Tick Data) पकड़ने का लूप
    while let Some(message) = read.next().await {
        match message {
            Ok(msg) => {
                if msg.is_text() {
                    let text = msg.into_text().unwrap();
                    
                    if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                        if let (Some(symbol), Some(price), Some(bid), Some(ask)) = (
                            parsed.get("symbol").and_then(|s| s.as_str()),
                            parsed.get("mark_price").and_then(|p| p.as_str()),
                            parsed.get("best_bid").and_then(|b| b.as_str()),
                            parsed.get("best_ask").and_then(|a| a.as_str())
                        ) {
                            // तीनों कॉइन्स का रॉ डेटा अलग-अलग निशानों के साथ
                            if symbol.contains("BTC") {
                                println!("🟠 [BTC-ENGINE] RAW: PRICE: {} | BID: {} | ASK: {}", price, bid, ask);
                            } else if symbol.contains("ETH") {
                                println!("🔵 [ETH-ENGINE] RAW: PRICE: {} | BID: {} | ASK: {}", price, bid, ask);
                            } else if symbol.contains("SOL") {
                                println!("🟣 [SOL-ENGINE] RAW: PRICE: {} | BID: {} | ASK: {}", price, bid, ask);
                            }
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
