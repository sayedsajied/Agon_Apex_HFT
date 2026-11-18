use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, hjb_vec[drift_mu, var_sigma2, opt_control_u])
static HJB_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
]);

// ENGINE 49: Tier-0 Hamilton-Jacobi-Bellman (HJB) Stochastic Optimal Control Solver
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

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = HJB_STATE.lock() {
        let (last_p, prev_hjb) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_hjb);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let spread_bps = (((best_ask - best_bid).max(0.0) / price) * 10000.0).max(0.1);

        // State evolution for drift (mu) and diffusion variance (sigma^2)
        let mu = (prev_hjb[0] * 0.84) + (ret_bps * 0.16);
        let diff = ret_bps - mu;
        let sigma_sq = ((prev_hjb[1] * 0.86) + (diff * diff * 0.14)).max(0.08);

        // HJB Value Function V(x) derivatives & execution cost penalty (eta)
        // First-Order Condition of Hamiltonian: sup_u [ u * mu * V_x - 0.5 * u^2 * (gamma * sigma^2 * V_xx + 2 * eta) ]
        let gamma_risk = 0.40;
        let eta_friction = spread_bps * 0.35;
        let hamiltonian_denom = (gamma_risk * sigma_sq) + (2.0 * eta_friction);

        let u_star_raw = (mu / hamiltonian_denom).clamp(-3.0, 3.0);
        let smooth_u = (prev_hjb[2] * 0.70) + (u_star_raw * 0.30);

        // Bellman Optimal Value Function Gain rate: J* = 0.5 * u*^2 * hamiltonian_denom
        let bellman_value_gain = 0.5 * smooth_u * smooth_u * hamiltonian_denom;
        let next_hjb = Vector3::new(mu, sigma_sq, smooth_u);

        let hjb_regime = if smooth_u > 0.65 {
            "🕹️ [HJB_OPTIMAL_THRUST_LONG_CONTROL]"
        } else if smooth_u < -0.65 {
            "🕹️ [HJB_OPTIMAL_THRUST_SHORT_CONTROL]"
        } else {
            "⚖️ [HJB_FRICTION_DAMPENED_HOLD]"
        };

        println!(
            "🎛️ [E49:{}] u*_CTRL:{:+.2} | V_GAIN:{:.2} | η_COST:{:.2}bps -> {}",
            symbol, next_hjb[2], bellman_value_gain, eta_friction, hjb_regime
        );

        state[idx] = (price, next_hjb);
    }
}
