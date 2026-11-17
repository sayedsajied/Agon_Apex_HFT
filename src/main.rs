use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

// === [PERMANENT MODULAR ENGINE SWITCHBOARD] ===
mod engine_02_scalp_burst;
mod engine_03_burst_velocity;
mod engine_04_orderbook_wall;
mod engine_05_vpin_toxic;
mod engine_06_hawkes_intensity;
mod engine_07_kyles_lambda;
mod engine_08_rolls_spread;
mod engine_09_ofi_tracker;
mod engine_10_iceberg_sniffer;
mod engine_11_whale_sweep;
mod engine_12_spoof_filter;
mod engine_13_micro_deviation;
mod engine_14_pressure_decay;
mod engine_15_funding_basis;
mod engine_16_kalman_matrix;
mod engine_17_itos_drift;
mod engine_18_ou_reversion;
mod engine_19_jump_diffusion;
mod engine_20_hurst_regime;
mod engine_21_hmm_switcher;
mod engine_22_shannon_entropy;
mod engine_23_wavelet_denoiser;
mod engine_24_mahalanobis_anomaly;
mod engine_25_eigen_covariance;
mod engine_26_lead_lag_matrix;
mod engine_27_cointegration_arb;
mod engine_28_garch_volatility;
mod engine_29_yang_zhang_vol;


















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
                    engine_04_orderbook_wall::process_tick(&parsed);
                    engine_05_vpin_toxic::process_tick(&parsed);
                    engine_06_hawkes_intensity::process_tick(&parsed);
                    engine_07_kyles_lambda::process_tick(&parsed);
                    engine_08_rolls_spread::process_tick(&parsed);
                    engine_09_ofi_tracker::process_tick(&parsed);
                    engine_10_iceberg_sniffer::process_tick(&parsed);
                    engine_11_whale_sweep::process_tick(&parsed);
                    engine_12_spoof_filter::process_tick(&parsed);
                    engine_13_micro_deviation::process_tick(&parsed);
                    engine_14_pressure_decay::process_tick(&parsed);
                    engine_15_funding_basis::process_tick(&parsed);
                    engine_16_kalman_matrix::process_tick(&parsed);
                    engine_17_itos_drift::process_tick(&parsed);
                    engine_18_ou_reversion::process_tick(&parsed);
                    engine_19_jump_diffusion::process_tick(&parsed);
                    engine_20_hurst_regime::process_tick(&parsed);
                    engine_21_hmm_switcher::process_tick(&parsed);
                    engine_22_shannon_entropy::process_tick(&parsed);
                    engine_23_wavelet_denoiser::process_tick(&parsed);
                    engine_24_mahalanobis_anomaly::process_tick(&parsed);
                    engine_25_eigen_covariance::process_tick(&parsed);
                    engine_26_lead_lag_matrix::process_tick(&parsed);
                    engine_27_cointegration_arb::process_tick(&parsed);
                    engine_28_garch_volatility::process_tick(&parsed);
                    engine_29_yang_zhang_vol::process_tick(&parsed);
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                    
                }
            }
        }
    }
}
