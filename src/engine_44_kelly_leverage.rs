use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, kelly_vec[mean_mu, var_sigma2, half_kelly_f])
static KELLY_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
]);

// ENGINE 44: Tier-0 Continuous-Time Merton-Kelly Optimal Fractional Leverage Sizer
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

    if let Ok(mut state) = KELLY_STATE.lock() {
        let (last_p, prev_k) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_k);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Exponentially weighted drift (mu) and variance (sigma^2) via nalgebra Vector3
        let mu = (prev_k[0] * 0.85) + (ret_bps * 0.15);
        let diff = ret_bps - mu;
        let sigma_sq = ((prev_k[1] * 0.85) + (diff * diff * 0.15)).max(0.10);

        // Net Edge after accounting for micro-horizon fee/hurdle rate (r_hurdle = 0.15 bps)
        let net_edge = if mu.abs() > 0.15 {
            mu.signum() * (mu.abs() - 0.15)
        } else {
            0.0
        };

        // Continuous Kelly Fraction: f* = net_edge / sigma^2, scaled by Half-Kelly (0.5) safety factor
        let full_kelly = net_edge / sigma_sq;
        let half_kelly = (0.5 * full_kelly).clamp(-5.0, 5.0);

        let k_vec = Vector3::new(mu, sigma_sq, half_kelly);
        let opt_lev = k_vec[2].abs();

        let kelly_regime = if k_vec[2] > 1.2 {
            "🟢 [KELLY_MAX_CONVICTION_LONG_SIZE]"
        } else if k_vec[2] < -1.2 {
            "🔴 [KELLY_MAX_CONVICTION_SHORT_SIZE]"
        } else if opt_lev < 0.15 {
            "🛡️ [KELLY_ZERO_EDGE_CAPITAL_SHIELD]"
        } else {
            "⚖️ [KELLY_FRACTIONAL_SCALP_SIZE]"
        };

        println!(
            "💰 [E44:{}] μ_EDGE:{:+.2}bps | σ²_VAR:{:.2} | HALF_KELLY_LEV:{:+.2}x -> {}",
            symbol, k_vec[0], k_vec[1], k_vec[2], kelly_regime
        );

        state[idx] = (price, k_vec);
    }
}
