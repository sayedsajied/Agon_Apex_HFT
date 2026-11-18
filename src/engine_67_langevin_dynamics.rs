use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, lang_vec[vel_v, drag_gamma, temp_kbt])
static LANGEVIN_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 0.5, 1.0)),
    (0.0, Vector3::new(0.0, 0.5, 1.0)),
    (0.0, Vector3::new(0.0, 0.5, 1.0)),
]);

// ENGINE 67: Tier-0 Langevin Stochastic Differential Equation & Fluctuation-Dissipation Engine
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

    if let Ok(mut state) = LANGEVIN_STATE.lock() {
        let (last_p, prev_l) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_l);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_depth = (bid_sz + ask_sz).max(1.0);

        // Viscous damping coefficient gamma proportional to passive orderbook thickness
        let inst_gamma = (book_depth.ln() * 0.22).clamp(0.10, 2.50);
        let gamma = (prev_l[1] * 0.82) + (inst_gamma * 0.18);

        // Langevin velocity update: dv = -gamma * v * dt + external_shock
        let v_prev = prev_l[0];
        let dv = (-gamma * v_prev * 0.25) + (ret_bps * 0.65);
        let v_next = (v_prev * 0.60) + (dv * 0.40);

        // Fluctuation-Dissipation Theorem: Thermal Energy k_B*T = m * E[v^2] & Einstein Diffusion D = k_B*T / gamma
        let inst_kbt = (ret_bps * ret_bps).max(0.05);
        let kbt = ((prev_l[2] * 0.85) + (inst_kbt * 0.15)).clamp(0.05, 30.0);
        let einstein_diff_d = kbt / gamma;

        let next_l = Vector3::new(v_next, gamma, kbt);

        let lang_regime = if v_next > 0.55 && einstein_diff_d > 1.8 {
            "🚀 [LANGEVIN_UNDERDAMPED_BALLISTIC_BULL]"
        } else if v_next < -0.55 && einstein_diff_d > 1.8 {
            "🩸 [LANGEVIN_UNDERDAMPED_BALLISTIC_BEAR]"
        } else if gamma > 1.25 && v_next.abs() < 0.25 {
            "🍯 [LANGEVIN_OVERDAMPED_VISCOUS_TRAP]"
        } else {
            "⚖️ [FLUCTUATION_DISSIPATION_BALANCE]"
        };

        println!(
            "🧪 [E67:{}] V_LANG:{:+.2} | γ_DRAG:{:.2} | D_EINSTEIN:{:.2} -> {}",
            symbol, next_l[0], next_l[1], einstein_diff_d, lang_regime
        );

        state[idx] = (price, next_l);
    }
}
