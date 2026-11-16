use serde_json::Value;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// हर कॉइन (BTC, ETH, SOL) की पिछली प्राइस और पिछले टिक का समय याद रखने की मेमोरी
static LAST_STATE: Mutex<[(f64, f64); 3]> = Mutex::new([(0.0, 0.0); 3]);

// ENGINE 03: Tier-0 Tick Velocity & Burst Acceleration Detector
pub fn process_tick(parsed: &Value) {
    if parsed.get("type").and_then(|t| t.as_str()) != Some("v2/ticker") {
        return;
    }

    let symbol = match parsed.get("symbol").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return,
    };

    let idx = if symbol.contains("BTC") {
        0
    } else if symbol.contains("ETH") {
        1
    } else if symbol.contains("SOL") {
        2
    } else {
        return;
    };

    let price_str = match parsed.get("mark_price").and_then(|p| p.as_str()) {
        Some(p) => p,
        None => return,
    };

    let price: f64 = price_str.parse().unwrap_or(0.0);
    if price <= 0.0 {
        return;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    if let Ok(mut state) = LAST_STATE.lock() {
        let (last_price, last_time) = state[idx];

        if last_price > 0.0 && now > last_time {
            let dt = now - last_time;
            // Basis Points (BPS) में प्रति सेकंड प्राइस भागने की रफ्तार (Velocity)
            let bps_move = ((price - last_price) / last_price) * 10000.0;
            let velocity = bps_move / dt;

            let burst_signal = if velocity > 8.0 {
                "🚀 [UPWARD_BURST_IGNITION]"
            } else if velocity < -8.0 {
                "🩸 [DOWNWARD_FLUSH_BURST]"
            } else {
                "📡 [NORMAL_FLOW]"
            };

            println!(
                "🌪️ [E03:{}] Δt:{:.3}s | MOVE:{:+.2}bps | VEL:{:+.2}bps/s -> {}",
                symbol, dt, bps_move, velocity, burst_signal
            );
        }

        state[idx] = (price, now);
    }
}
