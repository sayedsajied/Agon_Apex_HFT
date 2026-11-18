use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, k41_vec[macro_vel, micro_vel, epsilon_dissipation])
static KOLMOGOROV_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 0.0, 0.2)),
    (0.0, Vector3::new(0.0, 0.0, 0.2)),
    (0.0, Vector3::new(0.0, 0.0, 0.2)),
]);

// ENGINE 78: Tier-0 Kolmogorov K41 Turbulence -5/3 Energy Cascade & Dissipation Microscale Engine
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

    if let Ok(mut state) = KOLMOGOROV_STATE.lock() {
        let (last_p, prev_k) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_k);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Multi-scale eddy velocities: Integral Macro-Scale Velocity vs Micro-Tick Velocity
        let v_macro = (prev_k[0] * 0.92) + (ret_bps * 0.08);
        let v_micro = (prev_k[1] * 0.55) + (ret_bps * 0.45);
        let dv_eddy = (v_micro - v_macro).abs().max(0.05);

        // Kolmogorov K41 Four-Fifths Law Energy Dissipation Rate: epsilon ~ (delta_v)^3 / L_scale
        let inst_eps = (dv_eddy.powi(3) * 0.45).clamp(0.01, 50.0);
        let epsilon = (prev_k[2] * 0.78) + (inst_eps * 0.22);

        // Kinematic Viscosity nu from passive book depth & Kolmogorov Microscale eta = (nu^3 / epsilon)^(1/4)
        let nu_visc = (1.5 / (bid_sz + ask_sz).max(1.0).ln().max(0.5)).clamp(0.15, 2.5);
        let eta_microscale = ((nu_visc.powi(3)) / epsilon).powf(0.25).clamp(0.05, 10.0);

        // K41 Wavenumber Spectrum Energy E(k) = C_k * epsilon^(2/3) * k^(-5/3) at high tick frequency
        let tke_spectrum = 1.5 * epsilon.powf(2.0 / 3.0);
        let next_k = Vector3::new(v_macro, v_micro, epsilon);

        let k41_regime = if epsilon > 1.40 && v_micro > 0.45 {
            "🌪️ [K41_INERTIAL_CASCADE_BULL_BURST]"
        } else if epsilon > 1.40 && v_micro < -0.45 {
            "🩸 [K41_INERTIAL_CASCADE_BEAR_FLUSH]"
        } else if eta_microscale > 1.60 {
            "🧊 [K41_VISCOUS_DISSIPATION_SUBRANGE_CALM]"
        } else {
            "🌀 [RICHARDSON_EDDY_ENERGY_TRANSFER]"
        };

        println!(
            "🌀 [E78:{}] ε_DISS:{:.3} | η_SCALE:{:.3} | E(k):{:.2} -> {}",
            symbol, next_k[2], eta_microscale, tke_spectrum, k41_regime
        );

        state[idx] = (price, next_k);
    }
}
