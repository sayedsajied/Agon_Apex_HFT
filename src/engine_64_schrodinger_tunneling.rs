use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, q_vec[kinetic_e, barrier_v, tunnel_prob])
static SCHRODINGER_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.5, 1.5, 0.1)),
    (0.0, Vector3::new(0.5, 1.5, 0.1)),
    (0.0, Vector3::new(0.5, 1.5, 0.1)),
]);

// ENGINE 64: Tier-0 Schrodinger Wavefunction & WKB Quantum Barrier Tunneling Engine
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

    if let Ok(mut state) = SCHRODINGER_STATE.lock() {
        let (last_p, prev_q) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_q);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let spread_width = (((best_ask - best_bid).max(0.0) / price) * 10000.0).clamp(0.1, 10.0);

        // Wavepacket Kinetic Energy E = 0.5 * m * v^2 & Opposing Orderbook Potential Barrier V_0
        let inst_e = (0.5 * ret_bps * ret_bps).clamp(0.02, 25.0);
        let opposing_wall = if ret_bps >= 0.0 { ask_sz } else { bid_sz };
        let inst_v = (opposing_wall.max(1.0).ln() * 0.85).clamp(0.20, 15.0);

        let kin_e = (prev_q[0] * 0.78) + (inst_e * 0.22);
        let bar_v = (prev_q[1] * 0.85) + (inst_v * 0.15);

        // WKB Quantum Tunneling Transmission Coefficient: T = exp( -2 * width * sqrt(max(V_0 - E, 0)) )
        let barrier_deficit = (bar_v - kin_e).max(0.0);
        let wkb_exponent = (-0.45 * spread_width.sqrt() * barrier_deficit.sqrt()).clamp(-10.0, 0.0);
        let inst_tunnel = if kin_e >= bar_v { 0.99 } else { wkb_exponent.exp() };

        let tunnel_p = (prev_q[2] * 0.72) + (inst_tunnel * 0.28);
        let next_q = Vector3::new(kin_e, bar_v, tunnel_p);

        let q_regime = if tunnel_p > 0.62 && ret_bps > 0.15 {
            "⚛️ [QUANTUM_TUNNELING_BULL_WALL_BREACH]"
        } else if tunnel_p > 0.62 && ret_bps < -0.15 {
            "⚛️ [QUANTUM_TUNNELING_BEAR_WALL_BREACH]"
        } else if tunnel_p < 0.25 {
            "🧱 [WAVEFUNCTION_BARRIER_REFLECTION]"
        } else {
            "〰️ [SCHRODINGER_COHERENT_WAVE_PACKET]"
        };

        println!(
            "🌌 [E64:{}] E_KIN:{:.2} | V_WALL:{:.2} | P(TUNNEL):{:.1}% -> {}",
            symbol, next_q[0], next_q[1], next_q[2] * 100.0, q_regime
        );

        state[idx] = (price, next_q);
    }
}
