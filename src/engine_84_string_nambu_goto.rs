use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, str_vec[tension_t, ng_action, brane_res])
static STRING_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.5, 0.5, 0.0)),
    (0.0, Vector3::new(0.5, 0.5, 0.0)),
    (0.0, Vector3::new(0.5, 0.5, 0.0)),
]);

// ENGINE 84: Tier-0 String Theory, Nambu-Goto Worldsheet Tension & D-Brane Collision Engine
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

    let price = parsed
        .get("mark_price")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let quotes = parsed.get("quotes");
    let best_bid = quotes
        .and_then(|q| q.get("best_bid"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(price);
    let best_ask = quotes
        .and_then(|q| q.get("best_ask"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(price);
    let bid_sz = quotes
        .and_then(|q| q.get("bid_size"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);
    let ask_sz = quotes
        .and_then(|q| q.get("ask_size"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = STRING_STATE.lock() {
        let (last_p, prev_s) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_s);
            return;
        }

        // Worldsheet time derivative X_dot (price velocity) & spatial stretch X_prime (brane gap)
        let x_dot = ((price - last_p) / last_p) * 10000.0;
        let brane_gap = (((best_ask - best_bid).max(0.0) / price) * 10000.0).clamp(0.10, 10.0);
        let book_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Regge Slope alpha' inverse -> String Tension T = 1 / (2 * pi * alpha')
        let inst_tension = ((bid_sz + ask_sz).max(1.0).ln() * 0.38).clamp(0.25, 5.0);
        let tension_t = (prev_s[0] * 0.80) + (inst_tension * 0.20);

        // Nambu-Goto Worldsheet Area Lagrangian Density: L_NG = T * sqrt(1 + X_dot^2 + X_prime^2)
        let x_prime = brane_gap * 0.50;
        let ws_metric = (1.0 + (x_dot * x_dot) + (x_prime * x_prime)).sqrt();
        let inst_ng = (tension_t * ws_metric).clamp(0.10, 30.0);
        let ng_action = (prev_s[1] * 0.75) + (inst_ng * 0.25);

        // D-Brane Collision Tachyon Resonance: intensifies as brane_gap shrinks with directional thrust
        let inst_res = ((x_dot * 0.55) + (book_imb * 1.45)) / brane_gap.sqrt();
        let brane_res = ((prev_s[2] * 0.72) + (inst_res * 0.28)).clamp(-6.0, 6.0);

        let next_s = Vector3::new(tension_t, ng_action, brane_res);

        let string_regime = if ng_action > 1.85 && brane_res > 0.45 {
            "⚡ [BRANE_COLLISION_TACHYON_BULL_IGNITION]"
        } else if ng_action > 1.85 && brane_res < -0.45 {
            "🩸 [BRANE_COLLISION_TACHYON_BEAR_IGNITION]"
        } else if tension_t > 1.80 && brane_res.abs() <= 0.30 {
            "🎻 [NAMBU_GOTO_HIGH_TENSION_HARMONIC_COIL]"
        } else {
            "⚖️ [CLOSED_STRING_VACUUM_VIBRATION]"
        };

        println!(
            "🎻 [E84:{}] S_NG:{:.3} | T_STR:{:.2} | BRANE_RES:{:+.2} -> {}",
            symbol, next_s[1], next_s[0], next_s[2], string_regime
        );

        state[idx] = (price, next_s);
    }
}
