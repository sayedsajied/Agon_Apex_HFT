use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, bates_vec[stoch_var_v, jump_var_jv, cojump_score])
static BATES_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.8, 0.2, 0.0)),
    (0.0, Vector3::new(0.8, 0.2, 0.0)),
    (0.0, Vector3::new(0.8, 0.2, 0.0)),
]);

// ENGINE 53: Tier-0 Bates SVJ (Stochastic Volatility + Coupled Price-Vol Co-Jump Hybrid Engine)
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

    if let Ok(mut state) = BATES_STATE.lock() {
        let (last_p, prev_b) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_b);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let inst_var = (ret_bps * ret_bps).max(0.02);

        // Heston continuous diffusive variance component v_t (mean-reverting toward theta = 1.0)
        let v_prev = prev_b[0].max(0.05);
        let is_cojump = if inst_var > (3.5 * v_prev) && inst_var > 1.2 { 1.0 } else { 0.0 };

        // Bates Coupled Variance Jump J^v triggered simultaneously with Price Jump J^S
        let vol_jump_shock = if is_cojump > 0.5 { inst_var - v_prev } else { 0.0 };
        let v_next = ((v_prev * 0.88) + (inst_var.min(3.0 * v_prev) * 0.12)).clamp(0.05, 25.0);
        let jv_next = ((prev_b[1] * 0.82) + (vol_jump_shock * 0.18)).clamp(0.01, 50.0);

        // Signed Co-Jump Directional Impulse (Price Jump direction * Volatility Jump intensity)
        let total_bates_var = v_next + jv_next;
        let jump_dominance = jv_next / total_bates_var.max(0.05);
        let cojump_impulse = (prev_b[2] * 0.70) + ((ret_bps * jump_dominance) * 0.30);

        let next_b = Vector3::new(v_next, jv_next, cojump_impulse);

        let bates_regime = if jump_dominance > 0.40 && next_b[2] > 0.65 {
            "🚀 [BATES_SVJ_BULLISH_COJUMP_EXPLOSION]"
        } else if jump_dominance > 0.40 && next_b[2] < -0.65 {
            "🩸 [BATES_SVJ_BEARISH_COJUMP_CASCADE]"
        } else {
            "⚖️ [BATES_DIFFUSIVE_VOL_EQUILIBRIUM]"
        };

        println!(
            "🧬 [E53:{}] V_DIFF:{:.2} | J_VAR:{:.2} | SVJ_IMPULSE:{:+.2} -> {}",
            symbol, next_b[0], next_b[1], next_b[2], bates_regime
        );

        state[idx] = (price, next_b);
    }
}
