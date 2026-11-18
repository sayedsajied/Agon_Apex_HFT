use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, temp_t, energy_levels_vec4)
static BOLTZMANN_STATE: Mutex<[(f64, f64, Vector4<f64>); 3]> = Mutex::new([
    (0.0, 1.0, Vector4::new(1.0, 1.0, 1.0, 1.0)),
    (0.0, 1.0, Vector4::new(1.0, 1.0, 1.0, 1.0)),
    (0.0, 1.0, Vector4::new(1.0, 1.0, 1.0, 1.0)),
]);

// ENGINE 58: Tier-0 Boltzmann-Gibbs Helmholtz Free Energy & Thermodynamic Entropy Minimizer
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

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = BOLTZMANN_STATE.lock() {
        let (last_p, prev_t, prev_e) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 1.0, prev_e);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let temp_t = ((prev_t * 0.86) + (ret_bps.abs().max(0.1) * 0.14)).clamp(0.20, 8.0);

        // 4 State Potential Energies E_i [-2, -1, +1, +2]: lower energy = higher Boltzmann probability
        let state_grid = Vector4::new(-2.0, -0.7, 0.7, 2.0);
        let inst_e = state_grid.map(|s| ((s - ret_bps).powi(2) * 0.25).clamp(0.05, 15.0));
        let e_vec = (prev_e * 0.75) + (inst_e * 0.25);

        // Boltzmann Partition Function Z = sum exp(-E_i / T) & Gibbs Probabilities P_i
        let boltz_weights = e_vec.map(|e| (-e / temp_t).exp());
        let partition_z = boltz_weights.sum().max(1e-6);
        let prob_p = boltz_weights / partition_z;

        // Internal Energy U = sum(P_i * E_i), Gibbs Entropy S = -sum(P_i * ln P_i), Free Energy F = U - T*S
        let internal_u = prob_p.dot(&e_vec);
        let entropy_s = -prob_p.map(|p| if p > 1e-6 { p * p.ln() } else { 0.0 }).sum();
        let free_energy_f = internal_u - (temp_t * entropy_s);
        let work_bias = (prob_p[2] + prob_p[3]) - (prob_p[0] + prob_p[1]);

        let thermo_regime = if entropy_s < 1.05 && work_bias > 0.35 {
            "⚡ [BOLTZMANN_LOW_ENTROPY_BULL_WORK_RELEASE]"
        } else if entropy_s < 1.05 && work_bias < -0.35 {
            "🩸 [BOLTZMANN_LOW_ENTROPY_BEAR_COLLAPSE]"
        } else {
            "♨️ [GIBBS_MAXIMUM_ENTROPY_HEAT_EQUILIBRIUM]"
        };

        println!(
            "🌡️ [E58:{}] FREE_F:{:+.2} | ENTROPY_S:{:.3} | WORK_BIAS:{:+.2} -> {}",
            symbol, free_energy_f, entropy_s, work_bias, thermo_regime
        );

        state[idx] = (price, temp_t, e_vec);
    }
}
