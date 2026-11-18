use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_p, kramers_vec[state_x, noise_d, escape_rate_k])
static KRAMERS_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 1.0, 0.05)),
    (0.0, Vector3::new(0.0, 1.0, 0.05)),
    (0.0, Vector3::new(0.0, 1.0, 0.05)),
]);

// ENGINE 69: Tier-0 Kramers Escape Rate & Bistable Double-Well Potential Barrier Engine
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

    if let Ok(mut state) = KRAMERS_STATE.lock() {
        let (mut anchor_p, prev_k) = state[idx];

        if anchor_p <= 0.0 {
            state[idx] = (price, prev_k);
            return;
        }

        let ret_bps = ((price - anchor_p) / anchor_p) * 10000.0;
        anchor_p = (anchor_p * 0.93) + (price * 0.07);

        // Normalized reaction coordinate x in bistable potential V(x) = 0.25*a*x^4 - 0.5*b*x^2
        let x_coord = ((prev_k[0] * 0.75) + (ret_bps * 0.25)).clamp(-3.5, 3.5);
        let noise_d = ((prev_k[1] * 0.85) + ((ret_bps * ret_bps) * 0.15)).clamp(0.10, 25.0);

        // Double-well barrier height Delta_V = b^2 / (4*a) modulated by orderbook depth resistance
        let book_resist = ((bid_sz + ask_sz).max(1.0).ln() * 0.30).clamp(0.30, 3.0);
        let delta_v = (1.20 * book_resist).max(0.20);

        // Kramers Eyring-Kramers Transition Rate: k_esc = prefactor * exp(-Delta_V / D)
        let curvature_prefactor = 0.65;
        let inst_k = curvature_prefactor * (-delta_v / noise_d).exp();
        let k_esc = (prev_k[2] * 0.72) + (inst_k * 0.28);
        let mfpt_ticks = 1.0 / k_esc.max(1e-4);

        let next_k = Vector3::new(x_coord, noise_d, k_esc);

        let kramers_regime = if k_esc > 0.38 && x_coord > 0.40 {
            "🚀 [KRAMERS_SADDLE_CROSSING_BULL_WELL_TRANSITION]"
        } else if k_esc > 0.38 && x_coord < -0.40 {
            "🩸 [KRAMERS_SADDLE_CROSSING_BEAR_WELL_TRANSITION]"
        } else if k_esc < 0.12 {
            "🔒 [KRAMERS_METASTABLE_WELL_TRAPPED]"
        } else {
            "⚖️ [KRAMERS_BISTABLE_BARRIER_FLUCTUATION]"
        };

        println!(
            "⛰️ [E69:{}] k_ESC:{:.3} | ΔV_BARR:{:.2} | MFPT:{:.1}tks | X:{:+.2} -> {}",
            symbol, next_k[2], delta_v, mfpt_ticks, next_k[0], kramers_regime
        );

        state[idx] = (anchor_p, next_k);
    }
}
