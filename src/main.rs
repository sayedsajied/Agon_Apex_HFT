use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("🚀 AGON APEX HFT CORE - STARTING...");
        println!("⚙️ ENGINE 1: Delta Exchange WebSocket Connecting...");

            // डेल्टा एक्सचेंज का असली WebSocket URL
                let url = Url::parse("wss://production-ws.delta.exchange").unwrap();

                    // सर्वर से असली कनेक्शन बनाना
                        let (ws_stream, _) = connect_async(url).await.expect("❌ Connection Failed!");
                            println!("✅ Connected to Delta Exchange!");

                                let (mut write, mut read) = ws_stream.split();

                                    // BTC/USDT का लाइव टिक-डेटा मंगाने का असली पेलोड (Raw Data)
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

                                                                                                                                                                        // सर्वर को मैसेज भेजना
                                                                                                                                                                            use futures_util::SinkExt;
                                                                                                                                                                                write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
                                                                                                                                                                                    println!("📡 Subscribed to BTC/USDT Tick Data. Waiting for market action...\n");

                                                                                                                                                                                        // लाइव मार्केट डेटा को स्क्रीन पर बिना रुके प्रिंट करना
                                                                                                                                                                                            while let Some(message) = read.next().await {
                                                                                                                                                                                                    match message {
                                                                                                                                                                                                                Ok(msg) => {
                                                                                                                                                                                                                                if msg.is_text() {
                                                                                                                                                                                                                                                    println!("⚡ MARKET TICK: {}", msg.into_text().unwrap());
                                                                                                                                                                                                                                                                    }
                                                                                                                                                                                                                                                                                }
                                                                                                                                                                                                                                                                                            Err(e) => {
                                                                                                                                                                                                                                                                                                            println!("⚠️ Error receiving data: {:?}", e);
                                                                                                                                                                                                                                                                                                                            break;
                                                                                                                                                                                                                                                                                                                                        }
                                                                                                                                                                                                                                                                                                                                                }
                                                                                                                                                                                                                                                                                                                                                    }
                                                                                                                                                                                                                                                                                                                                                    }
                                                                                                                                                                                                                                                                                                                                                    