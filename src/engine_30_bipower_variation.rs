use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, prev_abs_ret, state_vec[RV, BV, Jump_Ratio])
static BPV_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.0, Vector3::new(0.5, 0.5, 0.0)),
    (0.0, 0.0, Vector3::new(0.5, 0.5, 0.0)),
    (0.0, 0.0, Vector3::new(0.5, 0.5, 0.0)),
]);

// ENGINE 30: Tier-0 Realized Bipower Variation (Continuous Vol vs. Discontinuous Jump Separator)
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

    if let Ok(mut state) = BPV_STATE.lock() {
        let (last_p, prev_abs_r, mut v_vec) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 0.0, v_vec);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let abs_r = ret_bps.abs();

        // Realized Variance (RV = r_t^2) vs Barndorff-Nielsen Bipower Variation (BV = (pi/2)*|r_t|*|r_{t-1}|)
        let inst_rv = abs_r * abs_r;
        let inst_bv = std::f64::consts::FRAC_PI_2 * abs_r * prev_abs_r;

        let smooth_rv = (v_vec[0] * 0.85) + (inst_rv * 0.15);
        let smooth_bv = (v_vec[1] * 0.85) + (inst_bv * 0.15);

        // Pure Discontinuous Jump Variance Component: J = max(RV - BV, 0)
        let jump_var = (smooth_rv - smooth_bv).max(0.0);
        let jump_share = if smooth_rv > 1e-4 {
            (jump_var / smooth_rv).clamp(0.0, 1.0)
        } else {
            0.0
        };

        v_vec = Vector3::new(smooth_rv, smooth_bv, jump_share);

        let bpv_regime = if jump_share > 0.45 && smooth_rv.sqrt() > 1.2 {
            "⚡ [BPV_PURE_JUMP_DOMINATED_FLOW]"
        } else if smooth_bv.sqrt() > 2.0 && jump_share <= 0.45 {
            "🌊 [BPV_SMOOTH_DIFFUSION_TREND]"
        } else {
            "⚖️ [BPV_QUIET_MICROSTRUCTURE]"
        };

        println!(
            "🔬 [E30:{}] RV_σ:{:.2} | BV_CONT_σ:{:.2} | JUMP_SHARE:{:.1}% -> {}",
            symbol,
            v_vec[0].sqrt(),
            v_vec[1].sqrt(),
            v_vec[2] * 100.0,
            bpv_regime
        );

        state[idx] = (price, abs_r, v_vec);
    }
}
