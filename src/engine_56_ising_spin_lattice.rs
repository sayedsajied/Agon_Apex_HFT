use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, spin_vec[ret_spin, book_spin, mag_m], temp_t)
static ISING_STATE: Mutex<[(f64, Vector3<f64>, f64); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 0.0, 0.0), 1.0),
    (0.0, Vector3::new(0.0, 0.0, 0.0), 1.0),
    (0.0, Vector3::new(0.0, 0.0, 0.0), 1.0),
]);

// ENGINE 56: Tier-0 Ising Ferromagnetic Spin Lattice & Phase-Transition Herd Detector
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

    if let Ok(mut state) = ISING_STATE.lock() {
        let (last_p, prev_spins, prev_temp) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_spins, 1.0);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Normalize micro-spins into [-1.0, +1.0] dipole states
        let s1_ret = (ret_bps * 1.5).tanh();
        let s2_book = (book_imb * 2.5).tanh();
        let s3_mom = ((prev_spins[0] + s1_ret) * 0.8).tanh();

        // Thermal Noise Temperature T (disordered volatility) vs Coupling Strength J = 1.25
        let spin_variance = ((s1_ret - s2_book).powi(2) + (s1_ret - s3_mom).powi(2)) * 0.5;
        let temp_t = ((prev_temp * 0.85) + (spin_variance * 0.15)).clamp(0.15, 3.0);
        let j_coupling = 1.25;

        // Mean-Field Ising Self-Consistent Equation: M = tanh( (J * M + h_ext) / T )
        let h_ext = (s1_ret + s2_book + s3_mom) / 3.0;
        let inst_mag = (((j_coupling * prev_spins[2]) + h_ext) / temp_t).tanh();
        let mag_m = (prev_spins[2] * 0.70) + (inst_mag * 0.30);

        // Ising Hamiltonian Energy H = -J * M^2 - h * M
        let hamiltonian_e = (-j_coupling * mag_m * mag_m) - (h_ext * mag_m);
        let next_spins = Vector3::new(s1_ret, s2_book, mag_m);

        let ising_regime = if mag_m > 0.68 && temp_t < 1.15 {
            "🧲 [ISING_FERROMAGNETIC_BULL_HERD_LOCK]"
        } else if mag_m < -0.68 && temp_t < 1.15 {
            "🧲 [ISING_FERROMAGNETIC_BEAR_HERD_LOCK]"
        } else if temp_t > 1.60 {
            "🔥 [ISING_PARAMAGNETIC_THERMAL_NOISE]"
        } else {
            "⚖️ [ISING_CURIE_CRITICAL_TRANSITION]"
        };

        println!(
            "🧲 [E56:{}] MAG_M:{:+.3} | TEMP_T:{:.2} | ENERGY_H:{:+.2} -> {}",
            symbol, next_spins[2], temp_t, hamiltonian_e, ising_regime
        );

        state[idx] = (price, next_spins, temp_t);
    }
}
