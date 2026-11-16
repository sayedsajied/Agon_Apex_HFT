use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

// === [PERMANENT MODULAR ENGINE SWITCHBOARD] ===
mod engine_02_scalp_burst;
mod engine_03_burst_velocity;

#[tokio::main]
async fn main() {
    println!("🚀 AGON APEX HFT CORE - TIER-0 SWITCHBOARD STARTING...");
    let url = Url::parse("wss://socket.delta.exchange").unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("❌ Connection Failed!");
    let (mut write, mut read) = ws_stream.split();

    let subscribe_msg = json!({
        "type": "subscribe",
        "payload": {
                        "channels": [
                {
                    "name": "v2/ticker",
                    "symbols": ["BTCUSDT", "ETHUSDT", "SOLUSDT"]
                },
                {
                    "name": "l2_orderbook",
                    "symbols": ["BTCUSDT", "ETHUSDT", "SOLUSDT"]
                }
            ]
            
        }
    });

    write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
    println!("📡 Live Stream Active! Routing ticks to Tier-0 Engines...");

    while let Some(message) = read.next().await {
        if let Ok(msg) = message {
            if msg.is_text() {
                if let Ok(parsed) = serde_json::from_str::<Value>(&msg.into_text().unwrap()) {
                    // Route raw tick data to Engine 02
                    engine_02_scalp_burst::process_tick(&parsed);
                    engine_03_burst_velocity::process_tick(&parsed);
                    
                }
            }
        }
    }
}
