use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, diff_var, jump_intensity)
static JUMP_STATE: Mutex<[(f64, f64, f64); 3]> = Mutex::new([(0.0, 1.0, 0.0); 3]);

// ENGINE 19: Tier-0 Merton Jump-Diffusion Discontinuous Burst Detector
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

    if let Ok(mut state) = JUMP_STATE.lock() {
        let (last_p, prev_var, prev_lambda) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 1.0, 0.0);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let sigma = prev_var.sqrt().max(0.2);

        // Merton Jump Threshold: |Return| > 2.8 * Continuous Diffusion Sigma
        let jump_ratio = ret_bps.abs() / sigma;
        let is_jump = if jump_ratio > 2.8 { 1.0 } else { 0.0 };

        // Update continuous diffusion variance (filtering out extreme jumps)
        let new_var = if is_jump > 0.0 {
            prev_var
        } else {
            ((prev_var * 0.88) + (ret_bps * ret_bps * 0.12)).max(0.05)
        };

        let new_lambda = (prev_lambda * 0.85) + (is_jump * 0.15);
        let merton_vec = Vector3::new(ret_bps, new_var.sqrt(), new_lambda);

        let jump_signal = if is_jump > 0.0 && ret_bps > 0.0 {
            "⚡ [MERTON_UPWARD_JUMP_SHOCK]"
        } else if is_jump > 0.0 && ret_bps < 0.0 {
            "⚡ [MERTON_DOWNWARD_JUMP_SHOCK]"
        } else {
            "🌊 [CONTINUOUS_DIFFUSION]"
        };

        println!(
            "💥 [E19:{}] RET:{:+.2}bps | DIFF_σ:{:.2} | JUMP_λ:{:.2} ({:.1}σ) -> {}",
            symbol, merton_vec[0], merton_vec[1], merton_vec[2], jump_ratio, jump_signal
        );

        state[idx] = (price, new_var, new_lambda);
    }
}
